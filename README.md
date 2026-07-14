# Gaussian Splatting Renderer

This high-performance rendering pipeline is built on native WebGPU abstractions via wgpu and features a customized local implementation of the wgpu-3dgs-viewer crate. It exposes a minimal WASM interface, designed for seamless integration into complex web-based environments.

---

> **This implementation serves as the core rendering engine of another project.** <br>
> **This is my associated publication:**

---

# Project Scope

The initial state of the project was a web-based Gaussian splatting
viewer built with Three.js for 3D graphics. The renderer was based on
SparkJS, which reads, parses, and displays pre-scanned .ply files. It
already included a camera controller and VR integration. There was also
a basic debug menu that was very helpful for finding and validating
possible optimization approaches.

The main bottleneck of the pipeline was the sorting of each splat for
every frame and the constant transfer of data between the GPU and CPU.
It is explained in the SparkJS documentation that the calculation of
distances from the camera to each splat is done on the graphics
processor. The individual sorting is performed on the CPU, before the
compressed data is then sent to the GPU again for rendering. Since WebGL
2.0, which is based on OpenGL ES 3.0, doesn't yet support compute
shaders or storage buffers, the data had to be sent back to the CPU
before rendering anyway.

This creates a bottleneck, which I observed in the browser's performance
runtime analysis. Over 30% of frame time was spent on `getBufferSubData`
calls (GPU to CPU) and another 20% on `texSubImage2D` calls (CPU to
GPU). This is less significant on machines with integrated GPUs and
shared memory, since the data transfer happens directly in RAM either
way. But I suspected the possibility for optimization on more powerful
computers to be very noticeable. Therefore, my final objective for this
project was the integration of a modern rendering pipeline that would
fix these exact problems.

First, I inspected the SparkJS renderer a little more, since the
prerelease version 2.0 was freshly released and brought some major
performance updates. This gave me a feel for Gaussian splat rendering,
which was a new technology for me. I experimented with LOD trees and
display settings, which provided a good reference for the maximum
performance that could be achieved with this kind of setup.

After that, I did all necessary preparation and research for my custom
rendering approach. I settled on Rust since it is the industry standard
for native speed in the web and can be easily compiled into WebAssembly.
I also had previous experience with wgpu, which is a very safe wrapper
crate that automatically targets the native graphics API without giving
away any low-level control. It is built with the WebGPU standard in
mind, making it the perfect fit for this project.

Additionally, I used the wgpu-3dgs-viewer crate, which implements
barebones Gaussian splatting functionality and integrates into any wgpu
project with minimal dependencies (which unfortunately means a lot of
boilerplate setup). This crate was not specifically made for web
integration, and even less for VR, leading to some issues that I will
discuss later.

After a lot of testing, changes, and VR integration attempts, I settled
on a fallback architecture. In this document, I will explain every
concept, implementation, and problem that occurred during this work.

# SparkJS Pipeline and LOD Optimization

## Implementation Details

At the start of the optimization, the focus was the SparkJS renderer
itself. The documentation had a chapter about performance tuning, which
was very helpful. I changed the `maxStdDev` parameter from its default
value of 8 to 3, which limits the Gaussian falloff. Another noteworthy
parameter was `clipXY`, which discards splats outside of screen space in
the fragment shader. After setting it to 1.0, which represents the exact
screen bounds, the effect became visible, especially when making fast
turns. I also tried changing the transparency falloff of the individual
splats, but none of those parameters seemed to have a big impact on
performance on my machine.

Everything was tested on my home computer, which has a Ryzen 7 5800X,
32GB of RAM, an RTX 4070 Ti, and 16GB of VRAM. I would estimate that the
effect could be greater on weaker machines, for example, laptops with an
iGPU. There, the bottleneck is shifted, and actual rendering
optimizations would be a more significant improvement. But since this
was not the main focus of my optimization plan, I moved on to the next
promising test.

<figure id="fig:placeholder" data-latex-placement="h">
<img src="./parameters.png" style="width:100.0%" />
<figcaption>Parameter Experimentation</figcaption>
</figure>

After deactivating the `autoUpdate` function of the renderer, I
implemented a threshold check that only updated SparkJS after the camera
moves. Theoretically, this would only sort every splat when the user
crosses a rotation or movement delta. Unfortunately, this also didn't
generate a big performance increase on my machine, because of the same
reasons stated before. But it was a good test, which I would use later
in the project to optimize my custom renderer. After eliminating the
sorting and data transfer bottlenecks, this change would become the
single biggest performance increase in the entire project.

``` {caption="Update Threshold"}
const deltaPos = camera.position.distanceTo(lastSortPos);
const deltaRot = camera.quaternion.angleTo(lastSortQuat);

if (deltaPos > MOVE_THRESHOLD || deltaRot > ROT_THRESHOLD_RAD) {
    spark.update({scene, camera});
    lastSortPos.copy(camera.position);
    lastSortQuat.copy(camera.quaternion);
}
```

Another experimental approach was to implement custom frustum culling
with so-called Dyno Shaders. This concept is SparkJS-specific and allows
the customization of splat processing. The idea was to cull the splats
outside of a specific NDC range as an extra step in the pipeline before
rendering. I implemented a `DynoFrustumCull` class following the SparkJS
documentation, but in the end, its behavior was extremely similar to the
`clipXY` parameter. Unfortunately, SparkJS abstracts a lot of the
pipeline, so I couldn't experiment with clipping in different stages.

The last and most promising optimization were the newly added level of
detail (LOD) features. SparkJS added a completely built-in LOD tree
system with the release of version 2.0. You simply had to set the `lod`
parameter to `true` when loading a mesh. This would lead to longer
loading times at the start of the application, but cause a notable FPS
boost. Different Foveate settings in the renderer object could be
configured, so the chunking would just affect splats behind or at the
edges of the camera.

There was also a Rust tool for pre-built LOD trees, which was somewhat
hidden in the GitHub preview branch of the library. With that, you could
generate the tree beforehand, giving you a chunked `.rad` file. The
loading time would decrease almost back to the original time, and the
positive effects of the optimization still remained. This concluded the
theoretical limit of the SparkJS optimization approach. The main
bottlenecks were not completely solved, but their impact was reduced.

## Performance Evaluation

The baseline performance was evaluated in the 4 million splat laboratory
scene on my home computer (Ryzen 7 5800X, RTX 4070 Ti). With an uncapped
framerate on Firefox, I reached around 140-180 fps when moving around
and 125 fps when viewing the whole scene from a specific angle. The
unoptimized scene required rendering and sorting around 8 million
triangles per frame.

Changing the base parameters of the SparkRenderer object caused a minor
gain of around 20 fps. After adding the rendering update with the
movement threshold, the performance only changed marginally. This
implies that SparkJS performs many calculations internally, independent
of the manual update call. The Dyno Frustum Culling also brought no
measurable performance gain. The Level of Detail implementation, on the
other hand, provided an increase of up to 60 fps, especially from camera
angles where you can see the whole scene. This is extremely impressive
when taking into consideration that I was using the biggest splat model
possible. I tried different LOD configurations, which I compared in the
following table.

| Metrik | Original | Live LOD | Pre-Build LOD | LOD Paging |
| :--- | :---: | :---: | :---: | :---: |
| **FPS (moving)** | 140 - 180 | 160 - 200 | 160 - 200 | 150 - 180 |
| **FPS (whole scene)** | 125 | 180 | 170 - 200 | 140 |
| **Loading Time** | 1s | 8s | 3s | 1s |
| **Triangles** | 8M | 2.5M - 4.5M | 1.5M - 3M | 1.5M - 3M |

The **Original** setup represents the unoptimized baseline, without any
LODs or other changes. While it loads the fastest, it must render all 8
million triangles, yielding the lowest framerate. Activating the basic
LOD implementation (**Live LOD**) by setting `lod:true` dynamically
builds the tree at the start of the application, which explains the long
8-second loading time. However, after the initial load, it is the most
stable out of all options, with no big framerate drops or fluctuations.

Evaluating the **Pre-Build LOD** approach shows the performance of an
optimized `.rad` file, generated beforehand using the Rust tool from the
SparkJS repository. The loading time is still higher than the original
since the browser has to prepare the LOD functionality, but bypassing
the tree generation saves a lot of time. Because the offline Rust tool
has more time to create a highly optimized version of the tree, the
number of triangles on screen is reduced by up to 1.5 million depending
on the camera position. Despite this, the framerate fluctuates more than
in the live setup.

Finally, the **LOD Paging** method divides the tree into chunks that get
dynamically loaded only when needed. The initial loading time is
extremely fast, but the performance is highly unstable and very
network-dependent. Furthermore, this approach creates over 10 different
files for the 4 million splat model that all have to live in the project
directory, which makes it much harder to manage.

Since this was just an exploratory step and not the actual main focus of
the project, I will not do a deeper performance analysis or test on
low-end devices. These results will simply be used as a baseline
comparison for the final custom renderer, which I will explain in the
following chapters.

# Custom Rust/WASM Splat Renderer

## Concept

test

## Implementation Details

## Performance Evaluation

# WebXR Integration Attempt

## Concept

## Failure Analysis

# Renderer Fallback Strategy

## Concept

## Implementation Details

## Performance Evaluation

# Conclusion

# Declaration of AI Usage

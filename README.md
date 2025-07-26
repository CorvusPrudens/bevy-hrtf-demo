# Bevy HRTF Demonstration

This repo demonstrates a straightforward HRTF integration via
[`bevy_seedling`](https://github.com/CorvusPrudens/bevy_seedling).
The HRTF data is stored in a SOFA file (`assets/sadie_h12.sofa`) and
extracted and rendered by the [`sofar`](https://docs.rs/sofar/latest/sofar/)
crate.

`sofar` is a wrapper around [`libmysofa`](https://github.com/hoene/libmysofa), a
C dependency. `sofar` also provides a simple renderer which is implemented in
pure Rust.

`sofar`'s renderer provides decent performance, permitting up to a couple
hundred individual spatial emitters at once on my M3 Macbook Pro. Here's a
plot of 16 emitters running at 44.1kHz/1024.

![16 HRTF emitters](https://github.com/CorvusPrudens/bevy-hrtf-demo/blob/master/profiling/16.png)

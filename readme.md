# Echolocation

Rust app that uses a HRTF to position microphone input in 3D space. The elevation, azimuth, and distance can be controlled via a GUI.

## Note

You would want to instantiate some sort of loopback device to capture this app's audio output into a microphone. This will depend on what platform you're using, I was able to accomplish this with Pipewire and qpwgraph.

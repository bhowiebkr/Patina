# Measurement source

`laser-core` and `laser-camera` come from Laser Level Webcam, commit
`2355302eb86c280089c8fecb3149ffafc80a87d4`.

The source repository is https://github.com/bhowiebkr/laser-level-webcam.
Each crate retains the MIT license. The numerical algorithms and their tests
are retained. The manifests use explicit metadata and local dependencies.
Text and comments use ASCII unit names and mathematical notation.

`measurement` adapts these crates to Patina values and workflows.
The synthetic camera reuses the beam profile across rows when its tilt is zero.
A regression test checks identical pixels against the general rendering path.

# What's new in LightCraft IR

## LightCraft IR 0.21.1-ir

### Dedicated Infrared sections in Edit
- Edit now groups IR workflows into dedicated IR-FALSE-COLOR (camera-bound IR DCP profile selector, WB & neutral pickers, 46 sRGB false-color mixes, vibrance/saturation), IR-MONO (infrared monochrome mixes + tone & contrast), and AEROCHROME (Chrome/Aerochrome DCPs, Foliage & Sky HSL, Primary Calibration).
- Switch cleanly between the full Edit stack and the streamlined Infrared workflow using the segmented Edit / Infrared bar at the top of Tools (or Window -> Infrared Panel, Shift+I).

### Automatic channel swap reset on profile switch
- Choosing a different camera profile automatically clears any active Channel Mixer IR swap or neutral-point pick calibrated for the previous profile, recorded in one Undo step.

### Extended sRGB IR mix catalog & hover preview
- Includes 46 clean-room sRGB infrared channel mixes with family filters, live hover preview across Edit and Infrared tabs, ir_gamut spectral highlight compression, and CPU/GPU parity.

### Camera-bound DCP profiles & RAW decoders
- Camera DCP profiles are filtered to each photo's camera model (UniqueCameraModel) and copied or pasted safely across different cameras.
- Includes measured compressed 12-bit Olympus E-M1 Mark II ORF decoding (ir_olympus12) and optional Nikon High Efficiency NEF decoding (--features nikon-he).

### Rebased on upstream LightCraft v0.21.1
- Rebuilt atop upstream LightCraft v0.21.1 with a distinct infrared praying mantis app icon and clearer Library upgraded catalog migration notices.

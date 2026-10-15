# Arcana CRT

Add `arcana-crt` to an Arcana project's plugin list to make the **CRT screen**
render job available.

The job reads one sampled `Image2D` from **Source** and creates one `Image2D`
at **Image**. `width` and `height` suggest the source resolution and default to
320 by 180 when nonpositive. The source image's actual resolution defines the
virtual CRT pixel grid during execution. The output suggests the configured
resolution and RGBA8 format while accepting stricter downstream requirements.

`curvature` is a nonnegative float. Zero is an exactly flat screen (infinite
curvature radius); larger values bend the screen further. The output contains
only the sampled, rounded luminous pixel cells. Curved corners and gaps between
cells are transparent, with no bezel or background. Source RGBA is treated as
premultiplied and the complete sampled value is multiplied by the pixel mask.

The source is requested only when the output is demanded. It requires sampled
usage; the output requires target usage.

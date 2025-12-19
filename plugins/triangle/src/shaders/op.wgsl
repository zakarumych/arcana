


struct Constants {
    op: u32,
}

var<push_constant> pc: Constants;
@group(0) @binding(0) var src : texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(1) var dst : texture_storage_2d<rgba8unorm, read_write>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let psrc = textureLoad(src, id.xy).rgb;
    let pdst = textureLoad(dst, id.xy).rgb;
    var pres = vec3(0.0);

    switch (pc.op) {
        case 0u: {
            pres = pdst + psrc;
        }
        case 1u: {
            pres = pdst - psrc;
        }
        case 2u: {
            pres = pdst * psrc;
        }
        case 3u: {
            pres = pdst / psrc;
        }
        default: {}
    }

    pres = clamp(pres, vec3(0.0), vec3(1.0));
    textureStore(dst, id.xy, vec4(pres, 1.0));
}   

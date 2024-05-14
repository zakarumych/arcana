


// Helper function to convert sRGB to linear RGB
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        return c / 12.92;
    } else {
        return pow((c + 0.055) / 1.055, 2.4);
    }
}

// Helper function to convert linear RGB to sRGB
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        return c * 12.92;
    } else {
        return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
    }
}

// Function to convert RGB to XYZ
fn rgb_to_xyz(rgb: vec3<f32>) -> vec3<f32> {
    // let linear_rgb: vec3<f32> = vec3<f32>(srgb_to_linear(rgb.r), srgb_to_linear(rgb.g), srgb_to_linear(rgb.b));
    let m: mat3x3<f32> = mat3x3<f32>(
        0.4124564,
        0.3575761,
        0.1804375,
        0.2126729,
        0.7151522,
        0.0721750,
        0.0193339,
        0.1191920,
        0.9503041
    );
    return m * rgb;
}

// Function to convert XYZ to RGB
fn xyz_to_rgb(xyz: vec3<f32>) -> vec3<f32> {
    let m: mat3x3<f32> = mat3x3<f32>(
        3.2404542,
        -1.5371385,
        -0.4985314,
        -0.9692660,
        1.8760108,
        0.0415560,
        0.0556434,
        -0.2040259,
        1.0572252
    );
    return m * xyz;
    // let linear_rgb: vec3<f32> = m * xyz;
    // return vec3<f32>(linear_to_srgb(linear_rgb.r), linear_to_srgb(linear_rgb.g), linear_to_srgb(linear_rgb.b));
}

fn xyz_to_lab_f(x: f32, ref_xyz: vec3<f32>) -> f32 {
    let t: f32 = x / ref_xyz.x;
    if t > 0.008856 {
        return pow(t, 1.0 / 3.0);
    } else {
        return 7.787 * t + 16.0 / 116.0;
    }
}

// Function to convert XYZ to L*a*b*
fn xyz_to_lab(xyz: vec3<f32>) -> vec3<f32> {
    let ref_xyz: vec3<f32> = vec3<f32>(0.95047, 1.0, 1.08883);

    let fx: f32 = xyz_to_lab_f(xyz.x, ref_xyz);
    let fy: f32 = xyz_to_lab_f(xyz.y, ref_xyz);
    let fz: f32 = xyz_to_lab_f(xyz.z, ref_xyz);

    let l: f32 = 116.0 * fy - 16.0;
    let a: f32 = 500.0 * (fx - fy);
    let b: f32 = 200.0 * (fy - fz);

    return vec3<f32>(l, a, b);
}

fn f_inv(t: f32) -> f32 {
    let t3: f32 = t * t * t;
    if t3 > 0.008856 {
        return t3;
    } else {
        return (t - 16.0 / 116.0) / 7.787;
    }
}

// Function to convert Lab* to XYZ
fn lab_to_xyz(lab: vec3<f32>) -> vec3<f32> {
    let ref_xyz: vec3<f32> = vec3<f32>(0.95047, 1.0, 1.08883);

    let fy: f32 = (lab.x + 16.0) / 116.0;
    let fx: f32 = lab.y / 500.0 + fy;
    let fz: f32 = fy - lab.z / 200.0;

    let x: f32 = ref_xyz.x * f_inv(fx);
    let y: f32 = ref_xyz.y * f_inv(fy);
    let z: f32 = ref_xyz.z * f_inv(fz);

    return vec3<f32>(x, y, z);
}

// Function to convert Lab* to LCH
fn lab_to_lch(lab: vec3<f32>) -> vec3<f32> {
    let l: f32 = lab.x;
    let c: f32 = sqrt(lab.y * lab.y + lab.z * lab.z);
    var h: f32 = atan2(lab.z, lab.y);

    if h < 0.0 {
        h = h + 2.0 * 3.14159265358979323846;
    }

    h = degrees(h);

    return vec3<f32>(l, c, h);
}

// Function to convert LCH to Lab*
fn lch_to_lab(lch: vec3<f32>) -> vec3<f32> {
    let l: f32 = lch.x;
    let a: f32 = lch.y * cos(radians(lch.z));
    let b: f32 = lch.y * sin(radians(lch.z));

    return vec3<f32>(l, a, b);
}

// Function to convert RGB to LCH
fn rgb_to_lch(rgb: vec3<f32>) -> vec3<f32> {
    let xyz: vec3<f32> = rgb_to_xyz(rgb);
    let lab: vec3<f32> = xyz_to_lab(xyz);
    let lch: vec3<f32> = lab_to_lch(lab);

    return lch;
}

// Function to convert LCH to RGB
fn lch_to_rgb(lch: vec3<f32>) -> vec3<f32> {
    let lab: vec3<f32> = lch_to_lab(lch);
    let xyz: vec3<f32> = lab_to_xyz(lab);
    let rgb: vec3<f32> = xyz_to_rgb(xyz);

    return rgb;
}

// fn linear_srgb_to_oklab(c: vec3<f32>) -> vec3<f32> {
//     let l = 0.4122214708f * c.r + 0.5363325363f * c.g + 0.0514459929f * c.b;
//     let m = 0.2119034982f * c.r + 0.6806995451f * c.g + 0.1073969566f * c.b;
//     let s = 0.0883024619f * c.r + 0.2817188376f * c.g + 0.6299787005f * c.b;

//     let l_ = pow(l, 1.0 / 3.0);
//     let m_ = pow(m, 1.0 / 3.0);
//     let s_ = pow(s, 1.0 / 3.0);

//     return vec3<f32>(
//         0.2104542553f * l_ + 0.7936177850f * m_ - 0.0040720468f * s_,
//         1.9779984951f * l_ - 2.4285922050f * m_ + 0.4505937099f * s_,
//         0.0259040371f * l_ + 0.7827717662f * m_ - 0.8086757660f * s_,
//     );
// }

// fn oklab_to_linear_srgb(c: vec3<f32>) -> vec3<f32> {
//     let l_ = c.x + 0.3963377774f * c.y + 0.2158037573f * c.z;
//     let m_ = c.x - 0.1055613458f * c.y - 0.0638541728f * c.z;
//     let s_ = c.x - 0.0894841775f * c.y - 1.2914855480f * c.z;

//     let l = l_ * l_ * l_;
//     let m = m_ * m_ * m_;
//     let s = s_ * s_ * s_;

//     return vec3<f32>(
//         4.0767416621f * l - 3.3077115913f * m + 0.2309699292f * s,
//         -1.2684380046f * l + 2.6097574011f * m - 0.3413193965f * s,
//         -0.0041960863f * l - 0.7034186147f * m + 1.7076147010f * s,
//     );
// }


struct VertOutput {
    @builtin(position)
    position: vec4<f32>,

    @location(0)
    color: vec3<f32>,
}

struct Constants {
    angle: f32,
    width: u32,
    height: u32,
}

var<push_constant> pc: Constants;
@group(0) @binding(0) var<uniform> colors: array<vec3<f32>, 3>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertOutput {
    // let colors = mat3x3<f32>(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(1.0, 0.0, 1.0));
    // var rgb: vec3<f32> = vec3<f32>(colors[0][index], colors[1][index], colors[2][index]);
    let rgb = colors[index];

    let x = (-0.5 + (f32(index) * 0.5));
    let y = (-(sqrt(3.0) / 6.0) + f32(index == 1u) * sqrt(3.0) / 2.0);

    let a = pc.angle * 6.28318530717958647692528676655900577;

    let ca = cos(a);
    let sa = sin(a);

    let output = VertOutput(
        vec4<f32>((ca * x + sa * y) / f32(pc.width) * f32(pc.height), (ca * y - sa * x), 0.0, 1.0),
        xyz_to_lab(rgb_to_xyz(rgb)),
    );
    return output;
}

@fragment
fn fs_main(@location(0) color: vec3<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(xyz_to_rgb(lab_to_xyz(color)), 1.0);
}

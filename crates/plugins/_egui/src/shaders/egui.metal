// language: metal2.0
#include <metal_stdlib>
#include <simd/simd.h>

using metal::uint;

struct VertInput {
    metal::float2 position;
    metal::float2 uv;
    metal::float4 color;
};
struct VertOutput {
    metal::float4 position;
    metal::float2 uv;
    metal::float4 color;
};
struct FragInput {
    metal::float2 uv;
    metal::float4 color;
};
struct PC {
    uint width;
    uint height;
    float scale;
};

metal::float3 linear_srgb_to_oklab(
    metal::float3 c
) {
    float l = ((0.4122214615345001 * c.x) + (0.5363325476646423 * c.y)) + (0.05144599452614784 * c.z);
    float m = ((0.21190349757671356 * c.x) + (0.6806995272636414 * c.y)) + (0.10739696025848389 * c.z);
    float s_1 = ((0.08830246329307556 * c.x) + (0.2817188501358032 * c.y)) + (0.6299787163734436 * c.z);
    float l_1 = metal::pow(l, 1.0 / 3.0);
    float m_1 = metal::pow(m, 1.0 / 3.0);
    float s_2 = metal::pow(s_1, 1.0 / 3.0);
    return metal::float3(((0.21045425534248352 * l_1) + (0.7936177849769592 * m_1)) - (0.004072046838700771 * s_2), ((1.9779984951019287 * l_1) - (2.4285922050476074 * m_1)) + (0.4505937099456787 * s_2), ((0.025904037058353424 * l_1) + (0.7827717661857605 * m_1)) - (0.8086757659912109 * s_2));
}

metal::float3 oklab_to_linear_srgb(
    metal::float3 c_1
) {
    float l_2 = (c_1.x + (0.3963377773761749 * c_1.y)) + (0.21580375730991364 * c_1.z);
    float m_2 = (c_1.x - (0.10556134581565857 * c_1.y)) - (0.0638541728258133 * c_1.z);
    float s_3 = (c_1.x - (0.08948417752981186 * c_1.y)) - (1.2914855480194092 * c_1.z);
    float l_3 = (l_2 * l_2) * l_2;
    float m_3 = (m_2 * m_2) * m_2;
    float s_4 = (s_3 * s_3) * s_3;
    return metal::float3(((4.076741695404053 * l_3) - (3.307711601257324 * m_3)) + (0.23096993565559387 * s_4), ((-1.2684379816055298 * l_3) + (2.609757423400879 * m_3)) - (0.34131938219070435 * s_4), ((-0.004196086432784796 * l_3) - (0.7034186124801636 * m_3)) + (1.7076146602630615 * s_4));
}

metal::float3 linear_from_srgb(
    metal::float3 srgb
) {
    metal::bool3 cutoff = srgb < metal::float3(0.040449999272823334);
    metal::float3 lower = srgb / metal::float3(12.920000076293945);
    metal::float3 higher = metal::pow((srgb + metal::float3(0.054999999701976776)) / metal::float3(1.0549999475479126), metal::float3(2.4000000953674316));
    return metal::select(higher, lower, cutoff);
}

metal::float3 srgb_from_linear_rgb(
    metal::float3 rgb
) {
    metal::bool3 cutoff_1 = rgb < metal::float3(0.0031308000907301903);
    metal::float3 lower_1 = rgb * metal::float3(12.920000076293945);
    metal::float3 higher_1 = (metal::float3(1.0549999475479126) * metal::pow(rgb, metal::float3(1.0 / 2.4000000953674316))) - metal::float3(0.054999999701976776);
    return metal::select(higher_1, lower_1, cutoff_1);
}

metal::float4 srgb_from_linear_rgba(
    metal::float4 linear_rgba
) {
    metal::float3 _e5 = srgb_from_linear_rgb(linear_rgba.xyz);
    return metal::float4(_e5, linear_rgba.w);
}

struct vs_mainInput {
    metal::float2 position [[attribute(0)]];
    metal::float2 uv [[attribute(1)]];
    metal::float4 color [[attribute(2)]];
};
struct vs_mainOutput {
    metal::float4 position [[position]];
    metal::float2 uv [[user(loc0), center_perspective]];
    metal::float4 color [[user(loc1), center_perspective]];
};
vertex vs_mainOutput vs_main(
  vs_mainInput varyings [[stage_in]]
, constant PC& pc [[user(fake0)]]
) {
    const VertInput input = { varyings.position, varyings.uv, varyings.color };
    VertOutput output = {};
    uint _e9 = pc.width;
    float _e13 = pc.scale;
    uint _e22 = pc.height;
    float _e26 = pc.scale;
    output.position = metal::float4((((input.position.x / static_cast<float>(_e9)) * _e13) * 2.0) - 1.0, (((input.position.y / static_cast<float>(_e22)) * _e26) * -2.0) + 1.0, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    VertOutput _e39 = output;
    const auto _tmp = _e39;
    return vs_mainOutput { _tmp.position, _tmp.uv, _tmp.color };
}


struct fs_main_linearInput {
    metal::float2 uv [[user(loc0), center_perspective]];
    metal::float4 color [[user(loc1), center_perspective]];
};
struct fs_main_linearOutput {
    metal::float4 member_1 [[color(0)]];
};
fragment fs_main_linearOutput fs_main_linear(
  fs_main_linearInput varyings_1 [[stage_in]]
, metal::sampler s [[user(fake0)]]
, metal::texture2d<float, metal::access::sample> t [[user(fake0)]]
) {
    const FragInput in = { varyings_1.uv, varyings_1.color };
    metal::float4 tex = t.sample(s, in.uv);
    metal::float4 _e6 = srgb_from_linear_rgba(tex);
    metal::float4 out_color_gamma = in.color * _e6;
    metal::float3 _e10 = linear_from_srgb(out_color_gamma.xyz);
    return fs_main_linearOutput { metal::float4(_e10, out_color_gamma.w) };
}


struct fs_main_srgbInput {
    metal::float2 uv [[user(loc0), center_perspective]];
    metal::float4 color [[user(loc1), center_perspective]];
};
struct fs_main_srgbOutput {
    metal::float4 member_2 [[color(0)]];
};
fragment fs_main_srgbOutput fs_main_srgb(
  fs_main_srgbInput varyings_2 [[stage_in]]
, metal::sampler s [[user(fake0)]]
, metal::texture2d<float, metal::access::sample> t [[user(fake0)]]
) {
    const FragInput in_1 = { varyings_2.uv, varyings_2.color };
    metal::float4 tex_1 = t.sample(s, in_1.uv);
    metal::float4 _e6 = srgb_from_linear_rgba(tex_1);
    metal::float4 out_color_gamma_1 = in_1.color * _e6;
    return fs_main_srgbOutput { out_color_gamma_1 };
}

fn load_texture(data: &[u8], _assets: &Assets) -> Result<LoadedTexture, TextureError> {
    let mut transcoder = basis_universal::Transcoder::new();

    if !transcoder.validate_header(&data) {
        return Err(TextureError::InvalidData);
    }

    match transcoder.basis_texture_type(&data) {
        basis_universal::BasisTextureType::TextureType2D => {
            let image_count = transcoder.image_count(&data);
            if image_count != 1 {
                return Err(TextureError::InvalidImageCount);
            }

            let info = transcoder.image_info(&data, 0).unwrap();

            let image_level_count = transcoder.image_level_count(&data, 0);
            if image_level_count == 0 {
                return Err(TextureError::NoImageLevels);
            }

            let mut level_offsets = SmallVec::new();
            let mut transcoded_bytes = Vec::new();

            for l in 0..image_level_count {
                if let Err(()) = transcoder.prepare_transcoding(&data) {
                    return Err(TextureError::NoImageLevels);
                }

                let result = transcoder.transcode_image_level(
                    data,
                    TranscoderTextureFormat::RGBA32,
                    TranscodeParameters {
                        image_index: 0,
                        level_index: l,
                        decode_flags: None,
                        output_row_pitch_in_blocks_or_pixels: None,
                        output_rows_in_pixels: None,
                    },
                );

                match result {
                    Err(TranscodeError::TranscodeFormatNotSupported) => {
                        return Err(TextureError::FormatNotSupported);
                    }
                    Err(TranscodeError::ImageLevelNotFound) => {
                        unreachable!();
                    }
                    Err(TranscodeError::TranscodeFailed) => return Err(TextureError::DecodeFailed),
                    Ok(bytes) => {
                        if l != 0 {
                            level_offsets.push(transcoded_bytes.len());
                        }
                        transcoded_bytes.extend_from_slice(&bytes);
                    }
                }
            }

            Ok(LoadedTexture {
                extent: Extent2::new(info.m_width, info.m_height),
                level_offsets,
                transcoded_bytes,
            })
        }
        _ => return Err(TextureError::ImageTypeNotSupported),
    }
}

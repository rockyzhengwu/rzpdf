use super::*;
#[test]
fn srgb_and_gray_profiles_convert_real_bytes_instead_of_alternate() {
    let srgb = ICCProfile::try_new(&ICCProfile::srgb().to_bytes().unwrap()).unwrap();
    let context = ColorConversionContext::default();
    for sample in [[0., 0., 0.], [1., 1., 1.], [0.2, 0.5, 0.8]] {
        let color = srgb.convert(&sample, &context).unwrap();
        for (actual, expected) in [color.r, color.g, color.b].into_iter().zip(sample) {
            assert!((actual - expected).abs() < 0.002, "{actual} {expected}");
        }
    }
    let gray = ICCProfile::gray_srgb().unwrap();
    assert_eq!(gray.number_of_components(), 1);
    let color = gray.convert(&[0.25], &context).unwrap();
    assert!(
        [color.r, color.g, color.b]
            .iter()
            .all(|v| (*v - 0.25).abs() < 0.003)
    );
    assert_eq!(srgb.inner.transforms.lock().unwrap().len(), 1);
}
#[test]
fn output_profile_roundtrip_is_explicit_and_invalid_dimensions_are_errors() {
    let adobe =
        ICCProfile::try_new(&moxcms::ColorProfile::new_adobe_rgb().encode().unwrap()).unwrap();
    let context = ColorConversionContext::with_output_profile(adobe.clone()).unwrap();
    let converted = ICCProfile::srgb()
        .convert(&[0.7, 0.3, 0.2], &context)
        .unwrap();
    let returned = adobe
        .convert(
            &[converted.r, converted.g, converted.b],
            &ColorConversionContext::default(),
        )
        .unwrap();
    assert!(
        (returned.r - 0.7).abs() < 0.003
            && (returned.g - 0.3).abs() < 0.003
            && (returned.b - 0.2).abs() < 0.003
    );
    assert!(ColorConversionContext::with_output_profile(ICCProfile::gray_srgb().unwrap()).is_err());
    assert!(adobe.convert(&[0.2], &context).is_err());
    assert!(adobe.convert(&[f32::NAN, 0., 0.], &context).is_err());
}
#[test]
fn profile_extent_and_neighbor_limits_fail_before_backend_and_retry() {
    let data = ICCProfile::srgb().to_bytes().unwrap();
    let limits = Limits {
        max_decoded_bytes: data.len(),
        ..Limits::default()
    };
    assert!(ICCProfile::try_new_with_limits(&data, limits).is_ok());
    assert!(matches!(
        ICCProfile::try_new_with_limits(
            &data,
            Limits {
                max_decoded_bytes: data.len() - 1,
                ..limits
            }
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    for length in [0, 4, 36, 128, 131, data.len() - 1] {
        assert!(ICCProfile::try_new(&data[..length]).is_err());
    }
    let mut bad = data.clone();
    bad[136..140].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(ICCProfile::try_new(&bad).is_err());
    bad = data.clone();
    bad[36] = 0;
    assert!(ICCProfile::try_new(&bad).is_err());
    assert!(ICCProfile::try_new(&data).is_ok());
}
#[test]
fn shared_transform_cache_is_bounded_and_thread_safe() {
    let source =
        ICCProfile::try_new(&moxcms::ColorProfile::new_adobe_rgb().encode().unwrap()).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let source = source.clone();
            scope.spawn(move || {
                for _ in 0..8 {
                    source
                        .convert(&[0.4, 0.3, 0.2], &ColorConversionContext::default())
                        .unwrap();
                }
            });
        }
    });
    assert_eq!(source.inner.transforms.lock().unwrap().len(), 1);
    for _ in 0..10 {
        let target = ICCProfile::try_new(&ICCProfile::srgb().to_bytes().unwrap()).unwrap();
        source
            .convert(
                &[0.4, 0.3, 0.2],
                &ColorConversionContext::with_output_profile(target).unwrap(),
            )
            .unwrap();
    }
    assert_eq!(source.inner.transforms.lock().unwrap().len(), 8);
}

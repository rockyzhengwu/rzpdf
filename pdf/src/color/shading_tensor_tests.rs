use super::*;

fn rectangle() -> [[u32; 2]; 16] {
    [
        [0, 0],
        [0, 85],
        [0, 170],
        [0, 255],
        [85, 255],
        [170, 255],
        [255, 255],
        [255, 170],
        [255, 85],
        [255, 0],
        [170, 0],
        [85, 0],
        [85, 85],
        [85, 170],
        [170, 170],
        [170, 85],
    ]
}
fn bytes(points: [[u32; 2]; 16], colors: [u8; 4]) -> Vec<u8> {
    let mut bytes = vec![0];
    for p in points {
        bytes.extend(p.map(|v| v as u8));
    }
    bytes.extend(colors);
    bytes
}
fn encoding() -> PatchEncoding<'static> {
    PatchEncoding {
        flag_bits: 8,
        coordinate_bits: 8,
        component_bits: 8,
        coordinates: [0., 255., 0., 255.],
        components: &[[0., 1.]],
    }
}
#[test]
fn tensor_stream_layout_and_interpolation_use_sixteen_controls() {
    let data = bytes(rectangle(), [0, 0, 255, 255]);
    assert_eq!(data.len(), 37);
    let mesh = decode_patch_stream_kind(&data, &encoding(), 4, usize::MAX, true).unwrap();
    let patch = &mesh.patches[0];
    assert_eq!(patch.geometry.point(0.25, 0.75), Some([63.75, 191.25]));
    let mut work = 100_000;
    let mut value = [-1.];
    let mut bounds = [Interval::exact(-1.)];
    assert!(
        mesh.sample([63.75, 191.25], 16, &mut work, &mut value, &mut bounds)
            .unwrap()
    );
    assert!((value[0] - 0.25).abs() < 1e-5);
    assert!(bounds[0].lower <= 0.25 && bounds[0].upper >= 0.25);
    assert_eq!(
        patch_record_layout_kind(0, 8, 8, 8, 1, true).unwrap().bits,
        37 * 8
    );
    assert_eq!(
        patch_record_layout_kind(1, 8, 8, 8, 1, true).unwrap().bits,
        27 * 8
    );
}
#[test]
fn tensor_interior_is_not_replaced_by_coons_boundary() {
    let mut points = rectangle();
    points[12][0] += 64;
    let geometry =
        CoonsPatch::from_tensor_record(0, &points, None, 8, [0., 255., 0., 255.]).unwrap();
    let coons = CoonsPatch::from_decoded_points(
        std::array::from_fn(|i| points[i]),
        8,
        [0., 255., 0., 255.],
    )
    .unwrap();
    assert_eq!(geometry.point(0.5, 0.5), Some([136.5, 127.5]));
    assert_eq!(coons.point(0.5, 0.5), Some([127.5, 127.5]));
    for (u, v) in [(0., 0.), (0., 1.), (1., 0.), (1., 1.)] {
        assert_eq!(geometry.point(u, v), coons.point(u, v));
    }
    assert_ne!(geometry.tensor_controls(), coons.tensor_controls());
    let h = 1e-5;
    let derivative = geometry.derivatives(0.4, 0.6).unwrap();
    for axis in 0..2 {
        let du = (geometry.point(0.4 + h, 0.6).unwrap()[axis]
            - geometry.point(0.4 - h, 0.6).unwrap()[axis])
            / (2. * h);
        let dv = (geometry.point(0.4, 0.6 + h).unwrap()[axis]
            - geometry.point(0.4, 0.6 - h).unwrap()[axis])
            / (2. * h);
        assert!((du - derivative[0][axis]).abs() < 1e-6);
        assert!((dv - derivative[1][axis]).abs() < 1e-6);
    }
}
#[test]
fn tensor_shared_flags_preserve_edge_and_corner_sources() {
    for flag in 1..=3 {
        let first = rectangle();
        let mut data = bytes(first, [0, 64, 128, 255]);
        data.push(flag);
        for point in &first[4..] {
            data.extend(point.map(|v| v as u8));
        }
        data.extend([32, 96]);
        let mesh = decode_patch_stream_kind(&data, &encoding(), 2, usize::MAX, true).unwrap();
        assert_eq!(mesh.patches.len(), 2);
        let previous = &mesh.patches[0];
        let next = &mesh.patches[1];
        for i in 0..4 {
            assert_eq!(
                next.geometry.points[i],
                previous.geometry.points[(flag as usize * 3 + i) % 12]
            );
        }
        for i in 0..2 {
            let old = (flag as usize + i) % 4;
            assert_eq!(next.values[i], previous.values[old]);
            assert_eq!(
                (next.bounds[i].lower, next.bounds[i].upper),
                (previous.bounds[old].lower, previous.bounds[old].upper)
            );
        }
        assert!(next.geometry.tensor_interior.is_some());
    }
}
#[test]
fn tensor_truncation_missing_predecessor_and_neighbor_budget_are_errors() {
    let data = bytes(rectangle(), [0, 0, 255, 255]);
    for len in 0..data.len() {
        assert!(decode_patch_stream_kind(&data[..len], &encoding(), 2, usize::MAX, true).is_err());
    }
    let mut bad = data.clone();
    bad[0] = 1;
    assert!(matches!(
        decode_patch_stream_kind(&bad, &encoding(), 2, usize::MAX, true),
        Err(PatchStreamError::MissingPreviousPatch)
    ));
    assert!(matches!(
        decode_patch_stream_kind(&data, &encoding(), 0, usize::MAX, true),
        Err(PatchStreamError::PatchLimit)
    ));
    let mut low = 0;
    let mut high = 100_000;
    while low < high {
        let mid = low + (high - low) / 2;
        if decode_patch_stream_kind(&data, &encoding(), 2, mid, true).is_ok() {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    assert!(low > 0);
    assert!(decode_patch_stream_kind(&data, &encoding(), 2, low, true).is_ok());
    assert!(matches!(
        decode_patch_stream_kind(&data, &encoding(), 2, low - 1, true),
        Err(PatchStreamError::MemoryLimit)
    ));
    assert!(decode_patch_stream_kind(&data, &encoding(), 2, low, true).is_ok());
}

#[test]
fn tensor_records_remain_bit_continuous_without_byte_alignment() {
    let mut bits = Vec::new();
    let mut push = |value: u32, width: usize| {
        for bit in (0..width).rev() {
            bits.push(((value >> bit) & 1) as u8);
        }
    };
    push(0, 2);
    for i in 0..16 {
        push((i % 2) as u32, 1);
        push(((i / 2) % 2) as u32, 1);
    }
    for v in [0, 1, 0, 1] {
        push(v, 1);
    }
    push(1, 2);
    for _ in 0..12 {
        push(0, 1);
        push(1, 1);
    }
    push(1, 1);
    push(0, 1);
    assert_eq!(bits.len(), 66);
    let mut data = vec![0u8; bits.len().div_ceil(8)];
    for (i, value) in bits.into_iter().enumerate() {
        data[i / 8] |= value << (7 - i % 8);
    }
    let encoding = PatchEncoding {
        flag_bits: 2,
        coordinate_bits: 1,
        component_bits: 1,
        coordinates: [0., 1., 0., 1.],
        components: &[[0., 1.]],
    };
    let plan = scan_patch_stream_kind(&data, 2, 1, 1, 1, 2, true).unwrap();
    assert_eq!(
        (plan.patches, plan.payload_bits, plan.tail_bits),
        (2, 66, 6)
    );
    let mesh = decode_patch_stream_kind(&data, &encoding, 2, usize::MAX, true).unwrap();
    assert_eq!(mesh.patches[1].values, [1., 0., 1., 0.]);
    let mut work = 0;
    let mut value = [-1.];
    let mut bounds = [Interval::exact(-1.)];
    assert!(matches!(
        mesh.sample([0.5, 0.5], 16, &mut work, &mut value, &mut bounds),
        Err(SearchError::WorkLimit)
    ));
    assert_eq!(value, [-1.]);
    assert_eq!((bounds[0].lower, bounds[0].upper), (-1., -1.));
}

#[test]
fn folded_tensor_certifies_all_roots_and_selects_greatest_u_for_common_v() {
    let mut points = rectangle();
    let locations = [
        (0, 0),
        (0, 1),
        (0, 2),
        (0, 3),
        (1, 3),
        (2, 3),
        (3, 3),
        (3, 2),
        (3, 1),
        (3, 0),
        (2, 0),
        (1, 0),
        (1, 1),
        (1, 2),
        (2, 2),
        (2, 1),
    ];
    for (p, (u, v)) in points.iter_mut().zip(locations) {
        *p = [[0, 255, 0, 128][u], [0, 85, 170, 255][v]];
    }
    let mesh = decode_patch_stream_kind(
        &bytes(points, [0, 0, 255, 255]),
        &encoding(),
        1,
        usize::MAX,
        true,
    )
    .unwrap();
    assert!(mesh.patches[0].geometry.proves_common_v());
    let mut low = 0.8;
    let mut high = 1.;
    for _ in 0..60 {
        let u = (low + high) * 0.5;
        let x = 765. * u - 1530. * u * u + 893. * u * u * u;
        if x < 110. {
            low = u;
        } else {
            high = u;
        }
    }
    let mut work = 1_000_000;
    let mut values = [-1.];
    let mut bounds = [Interval::exact(-1.)];
    assert!(
        mesh.sample([110., 127.5], 20, &mut work, &mut values, &mut bounds)
            .unwrap()
    );
    assert!((f64::from(values[0]) - (low + high) * 0.5).abs() < 1e-5);
    assert!(values[0] > 0.9);
}

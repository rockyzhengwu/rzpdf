use pdf::{
    budget::OperationBudget,
    page::content_program::{ContentProgram, OperationReplacement},
    parser::options::ParseOptions,
};

#[test]
fn batch_sequences_delimit_new_words_after_original_operand_delimiters() {
    let program =
        ContentProgram::from_decoded_bytes(b"17 mystery(A) Tj".to_vec(), Default::default())
            .unwrap();
    let pool = OperationBudget::new(1024 * 1024);
    let output = program
        .with_replaced_operation_sequences_with_budget(
            &[OperationReplacement {
                index: 1,
                source: b"q Q % end",
                operation_count: 2,
            }],
            Default::default(),
            &pool,
        )
        .unwrap();
    assert_eq!(output.as_ref().source_bytes(), b"17 mystery\nq Q % end\n");
    assert_eq!(
        output
            .as_ref()
            .operations()
            .iter()
            .map(|op| op.operator.name())
            .collect::<Vec<_>>(),
        ["mystery", "q", "Q"]
    );
    assert_eq!(
        output.as_ref().operations()[0]
            .operator
            .get_as_f32(0)
            .unwrap(),
        17.0
    );
}

#[test]
fn batch_page_commit_isolates_shared_content_and_rolls_back_all_events() {
    use pdf::{
        document::{PdfDocument, resource_manager::StandardFont},
        error::{PdfError, PdfResult},
        parser::syntax::ParseMode,
    };
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf 1 0 0 1 100 100 Tm (A) Tj (B) Tj (C) Tj ET").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 1).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let replacements = [
        OperationReplacement {
            index: 3,
            source: b"<58> Tj",
            operation_count: 1,
        },
        OperationReplacement {
            index: 4,
            source: b"[<> -600] TJ",
            operation_count: 1,
        },
    ];
    let before = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    let stamp = doc.stamp();
    let mut invalid = replacements;
    invalid[1].operation_count = 2;
    assert!(
        doc.replace_page_operation_sequences(0, &root, &invalid)
            .is_err()
    );
    assert!(doc.is_stamp_current(&stamp));
    let result: PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_page_operation_sequences(0, &root, &replacements)?;
        assert_ne!(
            editing.get_page(0)?.content_program()?.source_bytes(),
            root.source_bytes()
        );
        Err(PdfError::DocumentError("batch multi-event rollback".into()))
    });
    assert!(
        matches!(result, Err(PdfError::DocumentError(ref message)) if message == "batch multi-event rollback")
    );
    for page in [0, 1] {
        let current = doc.get_page(page).unwrap().content_program().unwrap();
        assert_eq!(current.source_bytes(), root.source_bytes());
        assert_eq!(current.stream_object_id(0), root.stream_object_id(0));
    }
    assert!(
        doc.replace_page_operation_sequences(0, &root, &replacements)
            .is_err()
    );
    let fresh = doc.get_page(0).unwrap().content_program().unwrap();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: PdfResult<()> = doc.edit_transaction(|editing| {
            editing.replace_page_operation_sequences(0, &fresh, &replacements)?;
            assert_ne!(
                editing.get_page(0)?.content_program()?.source_bytes(),
                fresh.source_bytes()
            );
            panic!("batch multi-event commit unwind");
        });
    }));
    assert_eq!(
        caught.unwrap_err().downcast_ref::<&str>().copied(),
        Some("batch multi-event commit unwind")
    );
    for page in [0, 1] {
        assert_eq!(
            doc.get_page(page)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
    }
    let retry = doc.get_page(0).unwrap().content_program().unwrap();
    doc.replace_page_operation_sequences(0, &retry, &replacements)
        .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for current in [&doc, &reopened] {
        assert_eq!(
            current
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
        let edited = current
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(
            edited
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "XC"
        );
        assert!((edited[1].position.x() - before[2].position.x()).abs() < 0.002);
        assert!((edited[1].position.y() - before[2].position.y()).abs() < 0.002);
    }
}

fn fixture() -> ContentProgram {
    ContentProgram::from_decoded_streams(
        vec![
            b"% header\n(A".to_vec(),
            Vec::new(),
            b") Tj 17 mystery\n[(B)".to_vec(),
            b"25(C)] TJ % tail\n(D) ".to_vec(),
            b"Tj\n% retained".to_vec(),
            b"\nq 1 0 0 1 0 0 cm Q\n".to_vec(),
            Vec::new(),
        ],
        Default::default(),
    )
    .unwrap()
}

fn replacements() -> [OperationReplacement<'static>; 3] {
    [
        OperationReplacement {
            index: 0,
            source: b"T*\n[<58> -600] TJ % fragment",
            operation_count: 2,
        },
        OperationReplacement {
            index: 2,
            source: b"[<> 25 <5A>] TJ",
            operation_count: 1,
        },
        OperationReplacement {
            index: 3,
            source: b"",
            operation_count: 0,
        },
    ]
}

#[test]
fn batch_reconstruction_preserves_cross_stream_neighbors_and_empty_chunks() {
    let options = ParseOptions::default();
    let original = fixture();
    let pool = OperationBudget::new(2 * 1024 * 1024);
    let output = original
        .with_replaced_operation_sequences_with_budget(&replacements(), options, &pool)
        .unwrap();
    let edited = output.as_ref();
    assert_eq!(edited.stream_ranges().len(), 7);
    assert_eq!(edited.stream_bytes(1), Some(b"".as_slice()));
    assert_eq!(edited.stream_bytes(6), Some(b"".as_slice()));
    assert_eq!(edited.stream_bytes(5), original.stream_bytes(5));
    assert_eq!(
        edited
            .operations()
            .iter()
            .map(|op| op.operator.name())
            .collect::<Vec<_>>(),
        ["T*", "TJ", "mystery", "TJ", "q", "cm", "Q"]
    );
    assert_eq!(edited.operations()[2].operator.get_as_f32(0).unwrap(), 17.0);
    assert_eq!(
        edited.operations()[1]
            .operator
            .get_as_array(0)
            .unwrap()
            .get(0)
            .unwrap()
            .as_string()
            .unwrap()
            .bytes(),
        b"X"
    );
    assert_eq!(
        edited.operations()[3]
            .operator
            .get_as_array(0)
            .unwrap()
            .get(2)
            .unwrap()
            .as_string()
            .unwrap()
            .bytes(),
        b"Z"
    );
    let lexical = std::str::from_utf8(edited.source_bytes()).unwrap();
    for token in ["% header\n", "% fragment\n", "% tail\n", "% retained\n"] {
        assert!(lexical.contains(token));
    }
    assert!(
        edited
            .stream_bytes(0)
            .unwrap()
            .starts_with(b"% header\n\nT*\n")
    );
    assert!(
        edited
            .stream_bytes(2)
            .unwrap()
            .ends_with(b"[<> 25 <5A>] TJ\n")
    );
    assert!(edited.stream_bytes(3).unwrap().starts_with(b" % tail\n"));
    assert!(edited.source_stamp().is_none());
    assert!(pool.live_bytes() > 0);
    drop(output);
    assert_eq!(pool.live_bytes(), 0);
    let copied = original
        .with_replaced_operation_sequences_with_budget(&[], options, &pool)
        .unwrap();
    assert_eq!(copied.as_ref().source_bytes(), original.source_bytes());
    assert_eq!(copied.as_ref().stream_ranges(), original.stream_ranges());
    drop(copied);
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn batch_reconstruction_rejects_bad_indices_counts_and_neighbor_operand_leaks() {
    let program = fixture();
    let options = ParseOptions::default();
    let pool = OperationBudget::new(2 * 1024 * 1024);
    let source = program.source_bytes().to_vec();
    let good = replacements();
    for invalid in [
        vec![good[1], good[0]],
        vec![good[0], good[0]],
        vec![OperationReplacement {
            index: 20,
            ..good[0]
        }],
        vec![OperationReplacement {
            operation_count: 1,
            ..good[0]
        }],
        vec![OperationReplacement {
            index: 0,
            source: b"q 99",
            operation_count: 1,
        }],
        vec![OperationReplacement {
            index: 0,
            source: b"[(unterminated",
            operation_count: 1,
        }],
    ] {
        assert!(
            program
                .with_replaced_operation_sequences_with_budget(&invalid, options, &pool)
                .is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(program.source_bytes(), source);
    }
    let mut limited = options;
    limited.limits.max_decoded_bytes = 1;
    assert!(
        program
            .with_replaced_operation_sequences_with_budget(&good, limited, &pool)
            .is_err()
    );
    limited = options;
    limited.limits.max_content_operations = 6;
    assert!(
        program
            .with_replaced_operation_sequences_with_budget(&good, limited, &pool)
            .is_err()
    );
    limited = options;
    limited.limits.max_raw_stream_bytes = 2;
    assert!(
        program
            .with_replaced_operation_sequences_with_budget(&good, limited, &pool)
            .is_err()
    );
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn batch_reconstruction_peak_rejection_retained_result_and_unwind_are_owned() {
    let program = fixture();
    let replacements = replacements();
    let options = ParseOptions::default();
    let probe = OperationBudget::new(2 * 1024 * 1024);
    let run = |pool: &OperationBudget| {
        program.with_replaced_operation_sequences_with_budget(&replacements, options, pool)
    };
    let output = run(&probe).unwrap();
    let expected = output.as_ref().source_bytes().to_vec();
    let peak = probe.peak_bytes();
    assert!(probe.live_bytes() > 0);
    drop(output);
    assert_eq!(probe.live_bytes(), 0);
    let pool = OperationBudget::new(peak + 6);
    let neighbor = pool.reserve(7).unwrap();
    assert!(run(&pool).is_err());
    assert_eq!(pool.live_bytes(), 7);
    drop(neighbor);
    let output = run(&pool).unwrap();
    let retained = pool.live_bytes();
    assert!(run(&pool).is_err());
    assert_eq!(pool.live_bytes(), retained);
    assert_eq!(output.as_ref().source_bytes(), expected);
    drop(output);
    assert_eq!(pool.live_bytes(), 0);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _output = run(&pool).unwrap();
        panic!("batch content result unwind");
    }));
    assert_eq!(
        caught.unwrap_err().downcast_ref::<&str>().copied(),
        Some("batch content result unwind")
    );
    assert_eq!(pool.live_bytes(), 0);
    drop(run(&pool).unwrap());
    assert_eq!(pool.live_bytes(), 0);
}

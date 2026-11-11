use super::{LAST_BIN, SizeHistogram, SizeSummary, bin_lower, bin_upper, quantile_exact, size_bin};

#[test]
fn bins_are_quarter_doublings() {
    assert_eq!(size_bin(0), 0);
    assert_eq!(size_bin(1), 1);
    // 2 = 2^(4/4): bins 2 to 4 hold no whole numbers.
    assert_eq!(size_bin(2), 5);
    assert_eq!(size_bin(1_024), 41);
    assert_eq!(size_bin(1_023), 40);
    assert_eq!(size_bin(u64::MAX), LAST_BIN);
}

#[test]
fn every_bin_holds_exactly_the_sizes_between_its_bounds() {
    for bin in 1..LAST_BIN {
        let (lower, upper) = (bin_lower(bin), bin_upper(bin).unwrap());
        assert!(lower <= upper, "bin {bin}");
        if lower < upper {
            assert_eq!(size_bin(lower), bin, "lower bound of bin {bin}");
            assert_eq!(size_bin(upper - 1), bin, "last size of bin {bin}");
        }
        assert!(size_bin(upper) > bin, "first size above bin {bin}");
    }
    assert_eq!(bin_upper(LAST_BIN), None);
    assert_eq!(size_bin(bin_lower(LAST_BIN)), LAST_BIN);
}

#[test]
fn histogram_percentiles_stay_within_the_bin_precision() {
    // A skewed spread of sizes, like real requests.
    let values: Vec<u64> = (1..=2_000_u64).map(|i| 200 + i * i * 7 % 90_000).collect();
    let mut histogram = SizeHistogram::default();
    for v in &values {
        histogram.add(size_bin(*v), 1);
    }
    let mut sorted = values.clone();
    sorted.sort_unstable();
    for p in [0.1, 0.5, 0.9, 0.99] {
        let exact = quantile_exact(&sorted, p).unwrap();
        let estimate = histogram.quantile(p).unwrap();
        assert!(
            (estimate / exact - 1.0).abs() < 0.1,
            "p{p}: estimate {estimate} vs exact {exact}"
        );
    }
}

#[test]
fn histograms_merge_exactly_and_empty_ones_have_no_percentiles() {
    let mut a = SizeHistogram::default();
    a.add(10, 3);
    let mut b = SizeHistogram::default();
    b.add(10, 2);
    b.add(12, 5);
    b.add(13, 0);
    a.merge(&b);
    assert_eq!(a.bins().collect::<Vec<_>>(), [(10, 5), (12, 5)]);
    assert_eq!(a.total(), 10);
    assert_eq!(SizeHistogram::default().quantile(0.5), None);
}

#[test]
fn a_single_bin_gives_percentiles_inside_it() {
    let mut histogram = SizeHistogram::default();
    histogram.add(41, 4);
    let (lower, upper) = (1_024.0, 1_218.0);
    for p in [0.0, 0.5, 1.0] {
        let value = histogram.quantile(p).unwrap();
        assert!((lower..=upper).contains(&value), "p{p}: {value}");
    }
}

#[test]
fn exact_quantiles_interpolate_between_ranks() {
    assert_eq!(quantile_exact(&[], 0.5), None);
    assert_eq!(quantile_exact(&[7], 0.9), Some(7.0));
    assert_eq!(quantile_exact(&[1, 2, 3, 4], 0.5), Some(2.5));
    let p90 = quantile_exact(&[1, 2, 3, 4], 0.9).unwrap();
    assert!((p90 - 3.7).abs() < 1e-9);
    let summary = SizeSummary::of_values(&[4, 1, 3, 2]);
    assert_eq!(summary.count, 4);
    assert_eq!(summary.mean, Some(2.5));
    assert_eq!(summary.p50, Some(2.5));
}

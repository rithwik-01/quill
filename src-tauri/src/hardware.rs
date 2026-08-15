//! hardware.rs — RAM tier → model recommendation (PLAN.md §8)

pub const MODEL_2B: &str = "qwen3.5:2b";
pub const MODEL_4B: &str = "qwen3.5:4b";
pub const MODEL_9B: &str = "qwen3.5:9b";
pub const MODEL_27B: &str = "qwen3.5:27b";

pub const TIERS: &[(f32, &str)] = &[
    (6.0, MODEL_2B),
    (12.0, MODEL_4B),
    (24.0, MODEL_9B),
    (f32::INFINITY, MODEL_27B),
];

#[inline]
pub fn tier_for_ram(gb: f32) -> &'static str {
    if !gb.is_finite() {
        return if gb == f32::INFINITY { MODEL_27B } else { MODEL_2B };
    }
    if gb < 6.0 {
        MODEL_2B
    } else if gb < 12.0 {
        MODEL_4B
    } else if gb <= 24.0 {
        MODEL_9B
    } else {
        MODEL_27B
    }
}

#[inline]
pub fn tier_for_total_ram(total_gb: f32) -> &'static str {
    tier_for_ram(total_gb * 0.7)
}

pub fn total_ram_gb() -> f32 {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_memory();
    let total = sys.total_memory();
    // sysinfo 0.32 returns KB on some platforms and bytes on others.
    // Heuristic: if total < 500M, treat as KB (max 128GB ~134M KB), else bytes.
    let bytes = if total < 500_000_000 {
        total * 1024
    } else {
        total
    };
    bytes as f32 / (1024.0 * 1024.0 * 1024.0)
}

pub fn recommended_model() -> String {
    tier_for_total_ram(total_ram_gb()).to_string()
}

#[tauri::command]
#[specta::specta]
pub fn get_recommended_model() -> String {
    recommended_model()
}

#[tauri::command]
#[specta::specta]
pub fn get_system_ram_gb() -> f32 {
    total_ram_gb()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tags_are_qwen35() {
        for &(_, tag) in TIERS {
            assert!(tag.starts_with("qwen3.5:"), "unexpected tag {tag}");
        }
    }

    #[test]
    fn below_6_goes_to_2b() {
        assert_eq!(tier_for_ram(0.0), MODEL_2B);
        assert_eq!(tier_for_ram(5.9), MODEL_2B);
        assert_eq!(tier_for_ram(5.999), MODEL_2B);
    }

    #[test]
    fn six_inclusive_goes_to_4b() {
        assert_eq!(tier_for_ram(6.0), MODEL_4B);
        assert_eq!(tier_for_ram(8.0), MODEL_4B);
    }

    #[test]
    fn twelve_inclusive_goes_to_9b() {
        assert_eq!(tier_for_ram(12.0), MODEL_9B);
        assert_eq!(tier_for_ram(20.0), MODEL_9B);
    }

    #[test]
    fn twenty_four_stays_in_9b() {
        assert_eq!(tier_for_ram(24.0), MODEL_9B);
    }

    #[test]
    fn above_24_goes_to_27b() {
        assert_eq!(tier_for_ram(24.0001), MODEL_27B);
        assert_eq!(tier_for_ram(32.0), MODEL_27B);
    }

    #[test]
    fn total_ram_helper_applies_0_7() {
        assert_eq!(tier_for_total_ram(8.0), MODEL_2B);
        assert_eq!(tier_for_total_ram(10.0), MODEL_4B);
        assert_eq!(tier_for_total_ram(20.0), MODEL_9B);
        assert_eq!(tier_for_total_ram(40.0), MODEL_27B);
    }
}

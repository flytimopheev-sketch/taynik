//! Генератор паролей-фраз (метод diceware).
//!
//! Фраза строится из слов встроенного словаря (1024 слова = 10 бит на слово),
//! поэтому её легко запомнить и продиктовать, а стойкость при этом выше,
//! чем у пароля той же длины из «кракозябр».
//!
//! 6 слов ≈ 60 бит, 8 слов ≈ 80 бит — с запасом выше типичных требований.

use rand::Rng;

use super::generator::Strength;

/// Встроенный словарь: 1024 слова, 10 бит энтропии на слово.
static WORDLIST: &str = include_str!("../../resources/wordlist.txt");

/// Число слов в словаре (степень двойки — важно для расчёта энтропии).
pub const WORDLIST_SIZE: usize = 1024;

/// Разделители между словами.
pub const SEPARATORS: &[&str] = &["-", ".", "_", " ", ""];

/// Настройки генератора парольной фразы.
#[derive(Debug, Clone)]
pub struct PassphraseOptions {
    /// Количество слов (3..=12).
    pub words: usize,
    /// Разделитель между словами.
    pub separator: String,
    /// Добавить после слов символ (например `!`).
    pub suffix: String,
    /// Добавить после слов число (добавляет ~10 бит).
    pub append_number: bool,
    /// Капитализировать каждое слово.
    pub capitalize: bool,
}

impl Default for PassphraseOptions {
    fn default() -> Self {
        Self {
            words: 6,
            separator: "-".to_string(),
            suffix: String::new(),
            append_number: false,
            capitalize: false,
        }
    }
}

/// Словарь как список слов.
fn words() -> Vec<&'static str> {
    WORDLIST
        .lines()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Энтропия фразы в битах (для указанного числа слов и добавок).
pub fn entropy_bits(opts: &PassphraseOptions) -> f64 {
    let mut bits = opts.words as f64 * (WORDLIST_SIZE as f64).log2();
    if opts.append_number {
        bits += 10.0; // 0..=9999
    }
    if !opts.suffix.is_empty() {
        bits += 5.0; // небольшой набор спецсимволов
    }
    bits
}

/// Оценка стойкости парольной фразы.
pub fn strength(opts: &PassphraseOptions) -> Strength {
    let bits = entropy_bits(opts);
    if bits < 50.0 {
        Strength::Weak
    } else if bits < 80.0 {
        Strength::Medium
    } else {
        Strength::Strong
    }
}

/// Сгенерировать парольную фразу.
pub fn generate(opts: &PassphraseOptions) -> Result<String, String> {
    if !(3..=12).contains(&opts.words) {
        return Err("Количество слов должно быть от 3 до 12".into());
    }
    if !SEPARATORS.contains(&opts.separator.as_str()) {
        return Err("Неизвестный разделитель слов".into());
    }
    let list = words();
    if list.is_empty() {
        return Err("Словарь пуст".into());
    }

    let mut rng = rand::thread_rng();
    let mut picked: Vec<&str> = Vec::with_capacity(opts.words);
    for _ in 0..opts.words {
        let idx = rng.gen_range(0..list.len());
        picked.push(list[idx]);
    }

    let mut phrase = picked
        .iter()
        .map(|w| {
            if opts.capitalize {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            } else {
                (*w).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(&opts.separator);

    if opts.append_number {
        // Фиксированная ширина в четыре цифры: число всегда добавляет
        // ровно 10 бит энтропии (0..9999) и не меняет длину фразы.
        phrase.push_str(&format!("{:04}", rng.gen_range(0..10_000)));
    }
    phrase.push_str(&opts.suffix);
    Ok(phrase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wordlist_has_expected_size() {
        let list = words();
        assert_eq!(list.len(), WORDLIST_SIZE);
        // Все слова уникальны и пригодны для набора.
        let mut unique: Vec<&str> = list.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), WORDLIST_SIZE);
        assert!(list.iter().all(|w| w.len() >= 3 && w.len() <= 12));
        assert!(list
            .iter()
            .all(|w| w.chars().all(|c| c.is_ascii_lowercase())));
    }

    #[test]
    fn generates_expected_number_of_words() {
        let opts = PassphraseOptions {
            words: 6,
            ..Default::default()
        };
        let phrase = generate(&opts).unwrap();
        assert_eq!(phrase.split('-').count(), 6);
    }

    #[test]
    fn respects_separator_and_case() {
        let opts = PassphraseOptions {
            words: 4,
            separator: ".".into(),
            capitalize: true,
            ..Default::default()
        };
        let phrase = generate(&opts).unwrap();
        assert_eq!(phrase.split('.').count(), 4);
        assert!(phrase.split('.').all(|w| w
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)));
    }

    #[test]
    fn appends_number_and_suffix() {
        let opts = PassphraseOptions {
            words: 3,
            append_number: true,
            suffix: "!".into(),
            ..Default::default()
        };
        for _ in 0..20 {
            let phrase = generate(&opts).unwrap();
            assert!(phrase.ends_with('!'));
            // Число добавляется ровно четырьмя цифрами.
            let digits: String = phrase
                .chars()
                .rev()
                .skip(1)
                .take_while(|c| c.is_ascii_digit())
                .collect();
            assert_eq!(digits.len(), 4, "фраза: {phrase}");
        }
    }

    #[test]
    fn rejects_invalid_options() {
        let opts = PassphraseOptions {
            words: 2,
            ..Default::default()
        };
        assert!(generate(&opts).is_err());

        let opts = PassphraseOptions {
            words: 4,
            separator: "#".into(),
            ..Default::default()
        };
        assert!(generate(&opts).is_err());
    }

    #[test]
    fn entropy_grows_with_words() {
        let six = PassphraseOptions {
            words: 6,
            ..Default::default()
        };
        let eight = PassphraseOptions {
            words: 8,
            ..Default::default()
        };
        assert!(entropy_bits(&eight) > entropy_bits(&six));
        assert!((entropy_bits(&six) - 60.0).abs() < 0.001);
    }
}

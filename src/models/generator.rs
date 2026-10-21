//! Генератор паролей и оценка стойкости.

use rand::Rng;

/// Стойкость пароля.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    Weak,
    Medium,
    Strong,
}

impl Strength {
    pub fn label(self) -> &'static str {
        match self {
            Strength::Weak => "Слабый",
            Strength::Medium => "Средний",
            Strength::Strong => "Стойкий",
        }
    }

    /// CSS-класс, соответствующий стойкости.
    pub fn css_class(self) -> &'static str {
        match self {
            Strength::Weak => "error",
            Strength::Medium => "warning",
            Strength::Strong => "success",
        }
    }
}

/// Настройки генерации.
#[derive(Debug, Clone)]
pub struct GeneratorOptions {
    pub length: usize,
    pub lowercase: bool,
    pub uppercase: bool,
    pub digits: bool,
    pub symbols: bool,
    /// Исключить похожие символы: l 1 I O 0 o
    pub exclude_similar: bool,
    /// Минимум цифр.
    pub min_digits: usize,
    /// Минимум спецсимволов.
    pub min_symbols: usize,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        Self {
            length: 20,
            lowercase: true,
            uppercase: true,
            digits: true,
            symbols: true,
            exclude_similar: true,
            min_digits: 2,
            min_symbols: 2,
        }
    }
}

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.<>?/~";
const SIMILAR: &str = "l1IoO0";

fn filter_set(set: &str, exclude_similar: bool) -> String {
    if exclude_similar {
        set.chars().filter(|c| !SIMILAR.contains(*c)).collect()
    } else {
        set.to_string()
    }
}

/// Сгенерировать пароль. Возвращает Err, если параметры противоречивы.
pub fn generate(opts: &GeneratorOptions) -> Result<String, String> {
    if !(8..=64).contains(&opts.length) {
        return Err("Длина пароля должна быть от 8 до 64 символов".into());
    }
    // Собираем наборы и сразу запоминаем позицию каждого, чтобы не считать
    // индексы набора по флагам: такой подход ломался, если набор
    // оказывался пустым после фильтрации.
    let mut sets: Vec<String> = Vec::new();
    let mut digits_idx: Option<usize> = None;
    let mut symbols_idx: Option<usize> = None;

    if opts.lowercase {
        sets.push(filter_set(LOWER, opts.exclude_similar));
    }
    if opts.uppercase {
        sets.push(filter_set(UPPER, opts.exclude_similar));
    }
    if opts.digits {
        digits_idx = Some(sets.len());
        sets.push(filter_set(DIGITS, opts.exclude_similar));
    }
    if opts.symbols {
        symbols_idx = Some(sets.len());
        sets.push(filter_set(SYMBOLS, false));
    }
    if sets.iter().all(|s| s.is_empty()) {
        return Err("Не выбран ни один набор символов".into());
    }

    let required =
        opts.min_digits * usize::from(opts.digits) + opts.min_symbols * usize::from(opts.symbols);
    if required > opts.length {
        return Err(format!(
            "Требуется минимум {required} цифр/спецсимволов, но длина пароля {}",
            opts.length
        ));
    }

    let mut rng = rand::thread_rng();
    // Важно: `chars().nth()` принимает номер символа, а не байта. Наборы
    // символов ASCII, но полагаться на это нельзя — считаем явно.
    let mut pick = |set: &str| -> char {
        let count = set.chars().count();
        if count == 0 {
            return 'a';
        }
        set.chars().nth(rng.gen_range(0..count)).unwrap_or('a')
    };

    // Обязательные символы: сначала минимумы по цифрам/спецсимволам,
    // затем хотя бы по одному из каждого выбранного набора.
    let mut chars: Vec<char> = Vec::new();
    if let Some(i) = digits_idx {
        for _ in 0..opts.min_digits {
            chars.push(pick(&sets[i]));
        }
    }
    if let Some(i) = symbols_idx {
        for _ in 0..opts.min_symbols {
            chars.push(pick(&sets[i]));
        }
    }
    for s in &sets {
        if !s.is_empty() {
            chars.push(pick(s));
        }
    }
    if chars.len() > opts.length {
        return Err("Слишком много обязательных символов для выбранной длины".into());
    }

    let all: String = sets.concat();
    while chars.len() < opts.length {
        chars.push(pick(&all));
    }

    // Перемешивание Фишера–Йетса.
    for i in (1..chars.len()).rev() {
        let j = rng.gen_range(0..=i);
        chars.swap(i, j);
    }
    Ok(chars.into_iter().collect())
}

/// Эвристическая оценка стойкости: длина + разнообразие символов.
pub fn strength(password: &str) -> Strength {
    let len = password.chars().count();
    let mut alphabet = 0usize;
    if password.chars().any(|c| c.is_ascii_lowercase()) {
        alphabet += 26;
    }
    if password.chars().any(|c| c.is_ascii_uppercase()) {
        alphabet += 26;
    }
    if password.chars().any(|c| c.is_ascii_digit()) {
        alphabet += 10;
    }
    if password.chars().any(|c| !c.is_ascii_alphanumeric()) {
        alphabet += 30;
    }
    if alphabet == 0 {
        return Strength::Weak;
    }
    // Энтропия в битах: len * log2(alphabet)
    let bits = len as f64 * (alphabet as f64).log2();
    if bits < 50.0 {
        Strength::Weak
    } else if bits < 80.0 {
        Strength::Medium
    } else {
        Strength::Strong
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn opts() -> GeneratorOptions {
        GeneratorOptions::default()
    }

    #[test]
    fn respects_length_and_charset() {
        for len in [8usize, 12, 20, 64] {
            let mut o = opts();
            o.length = len;
            let p = generate(&o).unwrap();
            assert_eq!(p.chars().count(), len);
            assert!(p.chars().all(|c| {
                c.is_ascii_alphanumeric() || "!@#$%^&*()-_=+[]{};:,.<>?/~".contains(c)
            }));
        }
    }

    #[test]
    fn honours_minimum_digits_and_symbols() {
        let mut o = opts();
        o.length = 20;
        o.min_digits = 5;
        o.min_symbols = 4;
        for _ in 0..50 {
            let p = generate(&o).unwrap();
            assert!(p.chars().filter(|c| c.is_ascii_digit()).count() >= 5);
            assert!(p.chars().filter(|c| !c.is_ascii_alphanumeric()).count() >= 4);
        }
    }

    #[test]
    fn excludes_similar_characters() {
        let mut o = opts();
        o.exclude_similar = true;
        for _ in 0..50 {
            let p = generate(&o).unwrap();
            assert!(!p.chars().any(|c| SIMILAR.contains(c)), "{p}");
        }
    }

    #[test]
    fn rejects_impossible_settings() {
        let mut o = opts();
        o.length = 4;
        assert!(generate(&o).is_err());

        let mut o = opts();
        o.length = 8;
        o.min_digits = 7;
        o.min_symbols = 7;
        assert!(generate(&o).is_err());

        let mut o = opts();
        o.lowercase = false;
        o.uppercase = false;
        o.digits = false;
        o.symbols = false;
        assert!(generate(&o).is_err());
    }

    #[test]
    fn generates_distinct_passwords() {
        let o = opts();
        let mut seen = HashSet::new();
        for _ in 0..200 {
            assert!(seen.insert(generate(&o).unwrap()));
        }
    }

    #[test]
    fn strength_levels() {
        assert_eq!(strength("abc"), Strength::Weak);
        // 11 строчных букв ≈ 52 бита — нижняя граница «среднего».
        assert_eq!(strength("abcdefghijk"), Strength::Medium);
        // 18 строчных букв ≈ 85 бит — надёжно.
        assert_eq!(strength("abcdefghijklmnopqr"), Strength::Strong);
        assert_eq!(strength("Xk9$mQ2#vLp7@rTn4%wYz8&"), Strength::Strong);
        assert_eq!(strength(""), Strength::Weak);
    }
}

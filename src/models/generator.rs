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
    let mut sets: Vec<String> = Vec::new();
    if opts.lowercase {
        sets.push(filter_set(LOWER, opts.exclude_similar));
    }
    if opts.uppercase {
        sets.push(filter_set(UPPER, opts.exclude_similar));
    }
    if opts.digits {
        sets.push(filter_set(DIGITS, opts.exclude_similar));
    }
    if opts.symbols {
        sets.push(filter_set(SYMBOLS, false));
    }
    if sets.iter().all(|s| s.is_empty()) {
        return Err("Не выбран ни один набор символов".into());
    }

    let required = opts.min_digits * opts.digits as usize + opts.min_symbols * opts.symbols as usize;
    if required > opts.length {
        return Err(format!(
            "Требуется минимум {required} цифр/спецсимволов, но длина пароля {}",
            opts.length
        ));
    }

    let mut rng = rand::thread_rng();
    let mut pick = |set: &str| -> char {
        set.chars().nth(rng.gen_range(0..set.len())).unwrap_or('a')
    };

    // Обязательные символы: сначала минимумы по цифрам/спецсимволам,
    // затем хотя бы по одному из каждого выбранного набора.
    let mut chars: Vec<char> = Vec::new();
    if opts.digits {
        let ds = &sets[opts.lowercase as usize + opts.uppercase as usize];
        for _ in 0..opts.min_digits {
            chars.push(pick(ds));
        }
    }
    if opts.symbols {
        let idx = opts.lowercase as usize + opts.uppercase as usize + opts.digits as usize;
        for _ in 0..opts.min_symbols {
            chars.push(pick(&sets[idx]));
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

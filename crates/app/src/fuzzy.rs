//! Matcher difuso de la paleta de comandos.
//!
//! Compara sobre una forma plegada (minúsculas y sin diacríticos) para que
//! «atención» encuentre «atencion» y viceversa, y devuelve los índices del
//! texto original para resaltar las coincidencias. El ranking es estable: a
//! igual puntuación manda el orden en que llegó la fuente.

/// Coincidencia de una consulta sobre un texto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FuzzyMatch {
    /// Puntuación; mayor es mejor.
    pub(crate) score: i32,
    /// Índices de caracteres del texto original que coinciden.
    pub(crate) indices: Vec<usize>,
}

/// Pliega un carácter a minúscula sin diacrítico.
fn fold(ch: char) -> char {
    match ch {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' | 'Å' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        other => other.to_lowercase().next().unwrap_or(other),
    }
}

/// `true` si el carácter separa palabras y por tanto un match ahí vale más.
fn is_boundary(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '-' | '_' | '.' | '/' | '\\' | ':' | '(' | ')' | '[' | ']' | ','
    )
}

/// Busca `query` como subsecuencia de `text`.
///
/// Devuelve `None` si falta algún carácter. La puntuación premia prefijos,
/// inicios de palabra y caracteres consecutivos; penaliza huecos largos.
pub(crate) fn score(query: &str, text: &str) -> Option<FuzzyMatch> {
    if query.trim().is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            indices: Vec::new(),
        });
    }

    let original: Vec<char> = text.chars().collect();
    let folded: Vec<char> = original.iter().copied().map(fold).collect();
    let needle: Vec<char> = query
        .chars()
        .map(fold)
        .filter(|ch| !ch.is_whitespace())
        .collect();
    if needle.is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            indices: Vec::new(),
        });
    }

    let mut indices = Vec::with_capacity(needle.len());
    let mut score = 0i32;
    let mut cursor = 0usize;
    let mut previous: Option<usize> = None;
    let mut first = true;

    for (needle_index, wanted) in needle.iter().enumerate() {
        let mut found = None;
        for (index, candidate) in folded.iter().enumerate().skip(cursor) {
            if candidate == wanted {
                found = Some(index);
                break;
            }
        }
        let index = found?;
        let gap = previous.map_or(0, |previous| index.saturating_sub(previous + 1));
        score -= (gap as i32) * 2;
        if index == 0 {
            score += 12;
        } else if is_boundary(folded[index - 1]) {
            score += 8;
        }
        if previous.is_some_and(|previous| index == previous + 1) {
            score += 6;
        }
        if first {
            score += 10;
            first = false;
        }
        // Un match al principio del texto con la consulta completa es el caso
        // más predecible: «tab» debe ganar a coincidencias dispersas.
        if needle_index == 0 && index == 0 {
            score += 6;
        }
        indices.push(index);
        previous = Some(index);
        cursor = index + 1;
    }

    if indices.len() == needle.len() && indices.last().is_some_and(|last| *last + 1 == folded.len())
    {
        score += 2;
    }
    Some(FuzzyMatch { score, indices })
}

/// Atajo para filtrar sin pedir los índices.
#[cfg(test)]
pub(crate) fn matches(query: &str, text: &str) -> bool {
    score(query, text).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents_fold_in_both_directions() {
        assert!(matches("atencion", "Atención"));
        assert!(matches("atención", "atencion"));
        assert!(matches("nunez", "Núñez"));
        assert!(matches("TELEFONO", "teléfono"));
        assert!(!matches("telefino", "teléfono"));
    }

    #[test]
    fn prefix_and_word_boundaries_outrank_scattered_matches() {
        let prefix = score("tab", "tab nueva").expect("coincide");
        let boundary = score("tab", "nueva tab").expect("coincide");
        let scattered = score("tab", "terminal abierta bash").expect("coincide");
        assert!(prefix.score > boundary.score);
        assert!(boundary.score > scattered.score);
        assert_eq!(prefix.indices, vec![0, 1, 2]);
        assert_eq!(boundary.indices, vec![6, 7, 8]);
    }

    #[test]
    fn ranking_is_stable_for_equal_scores() {
        let left = score("tab", "tab").expect("coincide");
        let right = score("tab", "tab").expect("coincide");
        assert_eq!(left, right);
    }

    #[test]
    fn empty_query_matches_everything_without_highlight() {
        let empty = score("  ", "lo que sea").expect("coincide");
        assert_eq!(empty.score, 0);
        assert!(empty.indices.is_empty());
    }
}

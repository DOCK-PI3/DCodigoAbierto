use nucleo::{Config, Nucleo, pattern::{CaseMatching, Normalization}};

/// Filtra `candidates` según `query` usando nucleo (fuzzy matching).
/// Retorna las rutas ordenadas por score descendente (nucleo ya las ordena).
pub fn fuzzy_filter(query: &str, candidates: &[String]) -> Vec<String> {
    if query.is_empty() {
        return candidates.to_vec();
    }

    let mut matcher = Nucleo::<String>::new(
        Config::DEFAULT,
        std::sync::Arc::new(|| {}),
        None,
        1,
    );

    let injector = matcher.injector();
    for path in candidates {
        let _ = injector.push(path.clone(), |s, cols| {
            cols[0] = s.clone().into();
        });
    }

    // Procesar items inyectados
    matcher.tick(10);

    // Parsear el patrón directamente con &str
    matcher.pattern.reparse(0, query, CaseMatching::Smart, Normalization::Smart, false);

    // Ticks para aplicar el filtro
    for _ in 0..20 {
        let status = matcher.tick(10);
        if !status.running {
            break;
        }
    }

    let snapshot = matcher.snapshot();
    snapshot
        .matched_items(0..snapshot.matched_item_count())
        .take(50)
        .map(|item| item.data.clone())
        .collect()
}


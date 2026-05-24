use nucleo::{Config, Nucleo, pattern::{CaseMatching, Normalization}};
use std::sync::Mutex;

/// Motor de fuzzy finding reusable.
/// Se construye una vez con todos los candidatos y se reutiliza en cada búsqueda.
pub struct FuzzyEngine {
    matcher: Mutex<Nucleo<String>>,
}

impl std::fmt::Debug for FuzzyEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FuzzyEngine").finish()
    }
}

impl FuzzyEngine {
    /// Construye el motor con la lista completa de candidatos.
    pub fn new(candidates: &[String]) -> Self {
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
        // Procesar todos los items inyectados
        for _ in 0..50 {
            let status = matcher.tick(10);
            if !status.running {
                break;
            }
        }
        Self { matcher: Mutex::new(matcher) }
    }

    /// Reconstruye el motor con nuevos candidatos (cuando el file tree cambia).
    /// NOTA: Esto reinicia el motor completamente. Si solo necesitas añadir, usa `new()`.
    #[allow(dead_code)]
    pub fn rebuild(&self, candidates: &[String]) {
        let mut matcher = self.matcher.lock().unwrap();
        matcher.restart(true);
        let injector = matcher.injector();
        for path in candidates {
            let _ = injector.push(path.clone(), |s, cols| {
                cols[0] = s.clone().into();
            });
        }
        for _ in 0..30 {
            let status = matcher.tick(10);
            if !status.running {
                break;
            }
        }
    }

    /// Filtra candidatos según `query` usando el motor preconstruido.
    /// Retorna las rutas ordenadas por score descendente (máx. 50).
    pub fn filter(&self, query: &str) -> Vec<String> {
        if query.is_empty() {
            // Si no hay query, devolvemos los primeros 50 candidatos
            let matcher = self.matcher.lock().unwrap();
            let snapshot = matcher.snapshot();
            return snapshot
                .matched_items(0..snapshot.matched_item_count())
                .take(50)
                .map(|item| item.data.clone())
                .collect();
        }

        let mut matcher = self.matcher.lock().unwrap();
        matcher.pattern.reparse(0, query, CaseMatching::Smart, Normalization::Smart, false);

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
}

/// Versión legacy (sin engine reutilizable) — útil para tests y casos simples.
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


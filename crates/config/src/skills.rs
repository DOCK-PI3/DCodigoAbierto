use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Una skill instalada (un directorio con un archivo SKILL.md).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Nombre de la skill (del frontmatter o del nombre del directorio)
    pub name: String,
    /// Descripción corta
    pub description: String,
    /// Ruta al directorio de la skill en disco
    pub path: PathBuf,
    /// Contenido del SKILL.md (instrucciones para el agente)
    pub instructions: String,
}

/// Gestor de skills para DCA.
///
/// Las skills se almacenan en `~/.config/dca/skills/`.
/// Se instalan desde repositorios de GitHub (mismo formato que skills.sh).
pub struct SkillsManager {
    skills_dir: PathBuf,
}

impl SkillsManager {
    /// Crea el gestor de skills.
    /// `skills_dir` es el directorio raíz donde se almacenan las skills.
    pub fn new(skills_dir: PathBuf) -> Self {
        Self { skills_dir }
    }

    /// Ruta por defecto: `~/.config/dca/skills/`
    pub fn default_dir() -> PathBuf {
        dirs_next::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dca")
            .join("skills")
    }

    /// Lista todas las skills instaladas.
    pub fn list_installed(&self) -> Result<Vec<Skill>> {
        if !self.skills_dir.exists() {
            return Ok(vec![]);
        }
        let mut skills = vec![];
        let entries = std::fs::read_dir(&self.skills_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                match self.parse_skill(&path, &skill_md) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => warn!("Error parseando skill en {:?}: {}", path, e),
                }
            }
        }
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    /// Instala una skill desde un repositorio de GitHub.
    ///
    /// `repo` tiene formato `owner/repo` (ej: `vercel-labs/skills`).
    /// Opcionalmente `skill_name` instala solo una skill específica del repo.
    pub async fn install_from_github(
        &self,
        repo: &str,
        skill_name: Option<&str>,
    ) -> Result<Vec<Skill>> {
        validate_repo_slug(repo)?;
        let repo_url = format!("https://github.com/{}.git", repo);
        let repo_name = repo.split('/').next_back().unwrap_or(repo);

        // Directorio temporal para clonar
        let temp_dir = std::env::temp_dir().join(format!("dca-skill-{}", repo_name));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }

        info!("Clonando {} en {:?}...", repo_url, temp_dir);

        // Clonar con git
        let clone_output = tokio::process::Command::new("git")
            .args(["clone", "--depth", "1", &repo_url])
            .arg(temp_dir.to_string_lossy().to_string())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await;

        match clone_output {
            Ok(output) if output.status.success() => {
                debug!("git clone exitoso para {}", repo);
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(color_eyre::eyre::eyre!(
                    "Error al clonar {}: {}",
                    repo_url,
                    stderr.trim()
                ));
            }
            Err(e) => {
                return Err(color_eyre::eyre::eyre!(
                    "No se pudo ejecutar git. ¿Está instalado? Error: {}",
                    e
                ));
            }
        }

        // Buscar SKILL.md en el repo clonado
        let mut installed = vec![];
        self.find_and_install_skills(&temp_dir, skill_name, &mut installed)?;

        // Limpiar temporal
        let _ = std::fs::remove_dir_all(&temp_dir);

        if installed.is_empty() {
            return Err(color_eyre::eyre::eyre!(
                "No se encontraron skills en {}. Asegúrate de que el repo contenga archivos SKILL.md.",
                repo
            ));
        }

        Ok(installed)
    }

    /// Busca skills recursivamente en un directorio y las instala.
    fn find_and_install_skills(
        &self,
        dir: &std::path::Path,
        filter_name: Option<&str>,
        installed: &mut Vec<Skill>,
    ) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        // Buscar SKILL.md en este directorio
        let skill_md = dir.join("SKILL.md");
        if skill_md.exists() {
            match self.parse_skill(dir, &skill_md) {
                Ok(skill) => {
                    let should_install = match filter_name {
                        Some(name) => skill.name == name,
                        None => true,
                    };
                    if should_install {
                        self.copy_skill(dir, &skill)?;
                        installed.push(skill);
                        return Ok(());
                    }
                }
                Err(e) => {
                    debug!("SKILL.md inválido en {:?}: {}", dir, e);
                }
            }
        }

        // Buscar en subdirectorios
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if name.starts_with('.') {
                        continue;
                    }
                    self.find_and_install_skills(&path, filter_name, installed)?;
                }
            }
        }

        Ok(())
    }

    /// Parsea un directorio de skill y extrae la metadata del SKILL.md.
    fn parse_skill(&self, dir: &std::path::Path, skill_md_path: &std::path::Path) -> Result<Skill> {
        let content = std::fs::read_to_string(skill_md_path)?;
        let (name, description, instructions) = parse_skill_md(&content, dir);

        Ok(Skill {
            name,
            description,
            path: dir.to_path_buf(),
            instructions,
        })
    }

    /// Copia una skill al directorio de skills de DCA.
    fn copy_skill(&self, src_dir: &std::path::Path, skill: &Skill) -> Result<()> {
        let safe_name = sanitize_skill_name(&skill.name)?;
        let dest_dir = self.skills_dir.join(&safe_name);
        if dest_dir.exists() {
            let _ = std::fs::remove_dir_all(&dest_dir);
        }
        std::fs::create_dir_all(&dest_dir)?;
        copy_dir_recursive(src_dir, &dest_dir)?;
        info!("Skill '{}' instalada en {:?}", skill.name, dest_dir);
        Ok(())
    }

    /// Elimina una skill instalada.
    pub fn remove_skill(&self, name: &str) -> Result<()> {
        let safe_name = sanitize_skill_name(name)?;
        let skill_dir = self.skills_dir.join(&safe_name);
        if skill_dir.exists() {
            std::fs::remove_dir_all(&skill_dir)?;
            info!("Skill '{}' eliminada", safe_name);
        } else {
            return Err(color_eyre::eyre::eyre!("Skill '{}' no encontrada", name));
        }
        Ok(())
    }

    /// Construye un fragmento de system prompt con todas las skills instaladas.
    pub fn build_skills_prompt(&self) -> Result<String> {
        let skills = self.list_installed()?;
        if skills.is_empty() {
            return Ok(String::new());
        }

        let mut prompt = String::from("\n\n## SKILLS DISPONIBLES\n");
        prompt.push_str("Tienes acceso a las siguientes skills especializadas. ");
        prompt.push_str("Úsalas cuando el usuario lo solicite o cuando la tarea lo requiera:\n\n");

        for skill in &skills {
            prompt.push_str(&format!("### Skill: {}\n", skill.name));
            prompt.push_str(&format!("{}\n", skill.description));
            prompt.push_str(&format!("```\n{}\n```\n\n", skill.instructions));
        }

        Ok(prompt)
    }
}

fn validate_repo_slug(repo: &str) -> Result<()> {
    let mut parts = repo.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    if owner.is_empty()
        || name.is_empty()
        || parts.next().is_some()
        || owner == "."
        || owner == ".."
        || name == "."
        || name == ".."
        || !owner.chars().all(is_safe_repo_char)
        || !name.chars().all(is_safe_repo_char)
    {
        return Err(color_eyre::eyre::eyre!(
            "Repositorio invalido '{}'. Usa el formato owner/repo.",
            repo
        ));
    }
    Ok(())
}

fn is_safe_repo_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.'
}

fn sanitize_skill_name(name: &str) -> Result<String> {
    let safe = name.trim();
    if safe.is_empty()
        || safe.contains('/')
        || safe.contains('\\')
        || safe == "."
        || safe == ".."
        || safe.chars().any(|ch| ch.is_control())
    {
        return Err(color_eyre::eyre::eyre!(
            "Nombre de skill invalido: '{}'",
            name
        ));
    }
    Ok(safe.to_string())
}

/// Parsea un archivo SKILL.md y extrae nombre, descripción e instrucciones.
fn parse_skill_md(content: &str, dir: &std::path::Path) -> (String, String, String) {
    let mut name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut description = String::new();
    let instructions: String;

    // Intentar extraer frontmatter YAML (entre --- y ---)
    if let Some(rest) = content.trim_start().strip_prefix("---") {
        if let Some(end) = rest.find("---") {
            let frontmatter = &rest[..end];
            let body = rest[end + 3..].trim();

            // Parsear líneas YAML simple
            for line in frontmatter.lines() {
                let line = line.trim();
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"').trim_matches('\'');
                    match key {
                        "name" => name = value.to_string(),
                        "description" => description = value.to_string(),
                        _ => {}
                    }
                }
            }

            instructions = truncate_instructions(body);
        } else {
            instructions = truncate_instructions(content);
        }
    } else {
        instructions = truncate_instructions(content);
    }

    if description.is_empty() {
        description = format!("Skill '{}'", name);
    }

    (name, description, instructions)
}

/// Trunca instrucciones muy largas para no saturar el prompt del modelo.
fn truncate_instructions(text: &str) -> String {
    const MAX_INSTRUCTIONS: usize = 8 * 1024;
    if text.len() <= MAX_INSTRUCTIONS {
        text.to_string()
    } else {
        // Encontrar el límite de carácter UTF-8 más cercano para no cortar a mitad de un carácter
        let mut bound = MAX_INSTRUCTIONS;
        while bound > 0 && !text.is_char_boundary(bound) {
            bound -= 1;
        }
        format!("{}...\n[Instrucciones truncadas a 8 KB]", &text[..bound])
    }
}

/// Copia un directorio recursivamente.
fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

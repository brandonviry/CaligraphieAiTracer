# Ordre de mission — Optimisation & Personnalisation post-compilation

## Objectif
Rendre l'application entièrement configurable sans recompiler.
Tout ce qui est aujourd'hui codé en dur dans le source doit devenir
un fichier de configuration lisible et modifiable à côté du .exe.

---

## Problèmes actuels (choses codées en dur)

| Endroit | Ce qui est dur-codé |
|---|---|
| `src/fonts.rs` ligne 6 | Liste des polices (`PRESET_FONTS` = tableau Rust) |
| `src/fonts.rs` ligne 19 | Chemin du cache polices (`assets/fonts`) |
| `src/app.rs` ligne 47-49 | Taille canvas (900×600), scale glyphe (220px) |
| `src/brush.rs` | Types de brosses uniquement programmatiques, pas de PNG externe |

---

## Solution : fichier `config.toml` à côté du .exe

Après compilation, l'utilisateur trouve dans le même dossier que le `.exe` :

```
CaligraphieAiTracer.exe
config.toml          ← éditable sans recompiler
assets/
  fonts/             ← cache TTF (auto-créé)
  brushes/           ← PNGs de brosses personnalisées (optionnel)
```

### Contenu de `config.toml`

```toml
[canvas]
width  = 900
height = 600
glyph_scale = 220.0

[server]
port = 7777
enabled = true

[defaults]
font      = "Dancing Script"
brush     = "brush_pen"
color     = [10, 10, 30]
thickness = 18.0
speed     = 4.0

[fonts]
# Polices Google Fonts disponibles dans le sélecteur.
# Ajouter une ligne pour en ajouter une nouvelle.
# Le fichier TTF sera téléchargé automatiquement au premier usage.
list = [
  "Dancing Script",
  "Great Vibes",
  "Pacifico",
  "Sacramento",
  "Caveat",
  "Pinyon Script",
  "Alex Brush",
  "Allura",
]
# Dossier de cache (relatif au .exe)
cache_dir = "assets/fonts"

[brushes]
# Brosses PNG personnalisées (fichiers dans assets/brushes/).
# L'app les charge au démarrage et les ajoute à la liste.
# Format : PNG 64×64, niveaux de gris, blanc = encre, noir = transparent.
custom = [
  # "assets/brushes/ma_brosse.png",
]
```

---

## Travail à faire

### 1. Ajouter la dépendance `toml`
```toml
# Cargo.toml
toml = "0.8"
```

### 2. Créer `src/config.rs`
- Struct `AppConfig` avec serde + toml
- `AppConfig::load()` → cherche `config.toml` à côté du .exe, sinon crée le fichier par défaut
- `AppConfig::save_default()` → écrit un `config.toml` exemple si absent

```rust
pub struct AppConfig {
    pub canvas: CanvasConfig,
    pub server: ServerConfig,
    pub defaults: DefaultsConfig,
    pub fonts: FontsConfig,
    pub brushes: BrushesConfig,
}
```

### 3. Modifier `src/fonts.rs`
- Supprimer `PRESET_FONTS` constant (codée en dur)
- `fonts_cache_dir()` lit depuis `AppConfig` plutôt que chemin fixe

### 4. Modifier `src/brush.rs`
- `load_custom_brushes(paths: &[String])` → charge les PNGs listés dans config
- Les ajoute à `default_brushes()`

### 5. Modifier `src/app.rs`
- `CalliApp::new(config: AppConfig)` au lieu de `Default`
- `CANVAS_W`, `CANVAS_H`, `GLYPH_SCALE` lus depuis config
- Liste polices et brosses construites depuis config

### 6. Modifier `src/main.rs`
- Charger config au démarrage
- Si `config.toml` absent → le créer avec valeurs par défaut + message dans la console
- Passer config à `CalliApp::new(config)`

### 7. Créer `config.toml` par défaut dans le repo
- Sera copié à côté du .exe lors du packaging (Étape 9)

---

## Comportement au démarrage (après optimisation)

```
1. L'app cherche config.toml dans le même dossier que le .exe
2. Si absent → crée config.toml avec valeurs par défaut + log "Config créée : config.toml"
3. Charge la config
4. Crée assets/fonts/ et assets/brushes/ si absents
5. Charge les brosses PNG custom listées dans config
6. Démarre l'UI avec les paramètres de la config
```

---

## Ce que l'utilisateur peut faire sans recompiler

- ✅ Ajouter n'importe quelle police Google Fonts → juste l'ajouter dans `config.toml [fonts] list`
- ✅ Ajouter une brosse PNG personnalisée → déposer le PNG dans `assets/brushes/` + ajouter le chemin dans config
- ✅ Changer la taille du canvas
- ✅ Changer le port TCP du serveur
- ✅ Changer les valeurs par défaut (couleur, épaisseur, vitesse)

---

## Ordre d'exécution

1. `src/config.rs` — struct + load/save
2. `Cargo.toml` — ajout `toml = "0.8"`
3. `config.toml` — fichier exemple dans le repo
4. `src/fonts.rs` — supprimer PRESET_FONTS, lire depuis config
5. `src/brush.rs` — chargement brosses PNG custom
6. `src/app.rs` — CalliApp::new(config)
7. `src/main.rs` — boot sequence

Validation : `cargo build --release` + lancer le .exe → `config.toml` créé automatiquement.

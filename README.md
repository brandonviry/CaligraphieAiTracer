# CaligraphieAiTracer

Application Rust qui simule un calligraphe en train d'écrire.
Choisissez le texte, la police Google Fonts, le type de brosse et la couleur —
l'application trace automatiquement, lettre par lettre, comme si un stylet physique écrivait avec pression et inclinaison réelles.

## Lancement rapide

```bash
cargo run --release
```

Ou téléchargez le `.exe` depuis les [releases GitHub](../../releases).

---

## Contrôle via script (TCP)

Un script externe peut envoyer des jobs de dessin via TCP sur `localhost:7777` :

```bash
# Job simple
python scripts/send_job.py "Bonjour" "Dancing Script"

# Avec options
python scripts/send_job.py "Hello" "Pacifico" --brush brush_pen --color 10 10 180 --thickness 20 --speed 6

# Séquence de démonstration (4 jobs enchaînés)
python scripts/send_job.py --demo
```

### Format JSON (une ligne par job)

```json
{
  "text": "Bonjour",
  "font": "Dancing Script",
  "brush": "brush_pen",
  "color": [10, 10, 30],
  "thickness": 18.0,
  "speed": 6.0,
  "clear_before": true
}
```

Tous les champs sauf `text` sont optionnels — les valeurs de `config.toml` sont utilisées par défaut.

---

## Types de brosses

| Identifiant        | Description                          |
|--------------------|--------------------------------------|
| `round_smooth`     | Cercle gaussien anti-aliasé          |
| `flat_calligraphy` | Rectangle incliné à 45°              |
| `brush_pen`        | Ellipse avec grain aléatoire         |
| `dry_ink`          | Texture arrachée, bords irréguliers  |

---

## Polices Google Fonts

Les polices sont téléchargées automatiquement à la première utilisation et mises en cache dans `assets/fonts/`.

Polices disponibles par défaut :
Dancing Script, Great Vibes, Pacifico, Sacramento, Caveat, Pinyon Script, Alex Brush, Allura

La liste est configurable dans `config.toml`.

---

## Configuration (`config.toml`)

Créé automatiquement au premier lancement à côté de l'exécutable.

```toml
[canvas]
width = 1200
height = 600
glyph_scale = 400.0

[server]
port = 7777
enabled = true

[defaults]
font = "Dancing Script"
brush = "brush_pen"
color = [10, 10, 30]
thickness = 18.0
speed = 6.0

[fonts]
list = ["Dancing Script", "Great Vibes", "Pacifico", "Sacramento",
        "Caveat", "Pinyon Script", "Alex Brush", "Allura"]
cache_dir = "assets/fonts"

[brushes]
custom = []   # chemins vers des PNG de brosses personnalisées
```

---

## Export / Rejouer

Dans le panneau gauche, section **Export / Rejouer** :

- **💾 Exporter** : sauvegarde le tracé courant en JSON (`tracé.json` par défaut)
- **▶ Rejouer** : charge un fichier JSON et rejoue l'animation exacte

---

## Architecture

```
Script externe (Python, JS…)
        │  JSON sur localhost:7777
        ▼
┌─────────────────────────────────────┐
│  CaligraphieAiTracer (Rust)         │
│                                     │
│  Thread TCP ──► VecDeque<Job>       │
│                      │              │
│              Thread UI (egui)       │
│                      │              │
│  Police TTF → Contours vectoriels   │
│         ↓                           │
│  Simulation pression gaussienne     │
│         ↓                           │
│  Tamponnage brosse bitmap           │
│         ↓                           │
│  Animation point par point          │
└─────────────────────────────────────┘
```

## Build

```bash
# Développement
cargo run

# Release (exe autonome ~8 Mo)
cargo build --release
# → target/release/calligraphie_ai_tracer.exe

# Tests
cargo test
```

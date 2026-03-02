# CaligraphieAiTracer — Plan de développement

## Concept
Application Rust qui simule un calligraphe en train d'écrire.
Un script externe envoie les paramètres (texte, police, brosse, couleur)
via TCP local → l'app dessine en temps réel avec animation de brosse bitmap.

---

## Architecture globale

```
Script externe (Python, JS, Claude Code...)
        │
        │  JSON sur localhost:7777
        ▼
┌─────────────────────────────────────┐
│  CaligraphieAiTracer (Rust)         │
│                                     │
│  Thread TCP ──► VecDeque<Job>       │
│                      │              │
│              Thread UI (egui)       │
│                      │              │
│         Pipeline de rendu :         │
│   Police TTF (Google Fonts cache)   │
│         ↓                           │
│   Contours vectoriels (ttf-parser)  │
│         ↓                           │
│   Simulation pression gaussienne    │
│         ↓                           │
│   Tamponnage brosse bitmap          │
│         ↓                           │
│   Animation point par point         │
└─────────────────────────────────────┘
```

---

## Format du message TCP (JSON)

```json
{
  "text": "Bonjour",
  "font": "Dancing Script",
  "brush": "brush_pen",
  "color": [10, 10, 30],
  "thickness": 18.0,
  "speed": 4.0,
  "clear_before": true
}
```

Champs optionnels → valeurs par défaut utilisées si absents.

---

## Types de brosses disponibles

| Identifiant      | Description                     |
|------------------|---------------------------------|
| `round_smooth`   | Cercle gaussien anti-aliasé     |
| `flat_calligraphy` | Rectangle incliné (45°)       |
| `brush_pen`      | Ellipse avec grain              |
| `dry_ink`        | Texture arrachée, bords irréguliers |

---

## Polices Google Fonts disponibles par défaut

Dancing Script, Great Vibes, Pacifico, Sacramento,
Caveat, Pinyon Script, Alex Brush, Allura

Téléchargées automatiquement à la première utilisation,
mises en cache dans `assets/fonts/`.

---

## Étapes de développement

### ✅ ÉTAPE 1 — Initialisation Cargo
Cargo.toml, src/main.rs, .gitignore. Fenêtre eframe vide.

### ✅ ÉTAPE 2 — Structure de données
`src/stroke.rs` : StrokePoint, Stroke, interpolation Catmull-Rom.
5 tests unitaires.

### ✅ ÉTAPE 3 — Extraction contours glyphes
`src/glyph.rs` : ttf-parser, Bézier → polylines.
`src/fonts.rs` : téléchargement Google Fonts + cache local.
14 tests unitaires.

### ✅ ÉTAPE 4 — Simulation calligraphique + brosses bitmap
`src/brush.rs` : 4 types de brosses, tamponnage bi-linéaire sur canvas RGBA.
`src/simulator.rs` : profil de pression gaussien, paint_stroke_on_canvas.
27 tests unitaires.

### ✅ ÉTAPE 5 — UI complète + moteur d'animation
`src/app.rs` : CalliApp, panneau contrôles, canvas egui, animation frame par frame.
`src/main.rs` : point d'entrée propre.
27 tests unitaires.

### ✅ ÉTAPE 6 — Serveur TCP + file d'attente
`src/server.rs` :
- Écoute sur `localhost:7777` dans un thread dédié
- Reçoit des messages JSON (format ci-dessus)
- Les pousse dans un `Arc<Mutex<VecDeque<DrawJob>>>`
- Le thread UI dépile et traite job par job

`scripts/send_job.py` :
- Script Python de test (aucune dépendance externe)
- Usage simple : `python send_job.py "Bonjour" Pacifico`
- Mode démo : `python send_job.py --demo` (4 jobs enchaînés)
- Options : --brush, --color, --thickness, --speed, --no-clear

### ✅ ÉTAPE 6b — File d'attente visible + gestion UI
`src/app.rs` — panneau file d'attente professionnel :
- Bouton **"Ajouter à la file"** : crée un DrawJob depuis les paramètres UI courants
- Bouton **"Tracer maintenant"** : priorité immédiate (vide la file + insère en tête)
- **Job EN COURS** affiché séparément (texte, police, brosse, couleur, épaisseur, vitesse)
- **Liste des jobs en attente** : résumé complet de chaque job + bouton [✕] individuel
- Bouton **"Vider la file"** : supprime tous les jobs en attente (pas celui en cours)
- Les jobs TCP entrants s'ajoutent à la même file et sont visibles dans l'UI

### ✅ ÉTAPE 6c — Modification et réordonnancement des jobs
`src/app.rs` — extensions de la file d'attente :
- **Bouton [✏]** sur chaque job : charge les paramètres du job dans le panneau de contrôle existant
  - Le panneau passe en mode "édition" (bandeau coloré + numéro du job édité)
  - L'utilisateur modifie via les contrôles habituels (texte, police, brosse, couleur, épaisseur, vitesse)
  - Bouton **"✓ Valider modification"** : écrase le job dans la file et sort du mode édition
  - Bouton **"✗ Annuler"** : restaure les paramètres UI d'avant et sort du mode édition
- **Boutons [▲] / [▼]** sur chaque job : monte ou descend dans la file
  - [▲] désactivé pour le premier job, [▼] désactivé pour le dernier
  - Swap de position dans le VecDeque

### ✅ ÉTAPE 7 — Export / Import JSON du tracé
`src/recorder.rs` :
- Format JSON `{ "version": 1, "strokes": [...] }`
- `export(strokes, path)` → sérialise et écrit le fichier
- `import(path)` → désérialise et valide la version
- 5 tests unitaires (roundtrip, version invalide, JSON invalide, vide)

`src/app.rs` — panneau Export / Rejouer :
- Champ texte pour le chemin du fichier (défaut : `tracé.json`)
- Bouton **"💾 Exporter"** : désactivé si canvas vide
- Bouton **"▶ Rejouer"** : charge le fichier et relance l'animation exacte, désactivé pendant animation

### ✅ ÉTAPE 8 — Tests unitaires & CI
- 41 tests unitaires répartis dans tous les modules
- `.github/workflows/ci.yml` : `cargo test --all` + `cargo build --release` + upload artifact sur push/PR

### ✅ ÉTAPE 9 — Packaging
- `cargo build --release` → `calligraphie_ai_tracer.exe` (~7,8 Mo, autonome)
- `README.md` : lancement, TCP, brosses, polices, config.toml, export/replay, architecture, build
- `scripts/package.ps1` : script de packaging reproductible
  - Usage : `.\scripts\package.ps1` (avec tests) ou `.\scripts\package.ps1 -SkipTests`
  - Option `-Version "1.2.0"` pour forcer un numéro de version
  - Produit `dist\CaligraphieAiTracer-<version>\` + `.zip` (3,4 Mo)
  - Contenu : exe renommé, config.toml, README.md, send_job.py, polices TTF cachées, brosses PNG
  - Entièrement reproductible : recrée le dossier depuis zéro à chaque appel

### 🔲 ÉTAPE 10 — Git & Publication
- Repo : `https://github.com/brandonviry/CaligraphieAiTracer.git`
- `.gitignore` : exclut `target/`, `assets/fonts/`, `dist/`, `analyse/`, `idéé.xml`
- `Cargo.lock` commité (binaire → reproductibilité garantie)
- Premier commit : tout le code source propre, sans `dist/` ni polices TTF
- Push sur `main` sans signature automatique

---

## Fichiers du projet

| Fichier | Rôle |
|---|---|
| `src/main.rs` | Point d'entrée |
| `src/app.rs` | UI egui + moteur animation |
| `src/stroke.rs` | Modèle de données StrokePoint/Stroke |
| `src/glyph.rs` | Extraction contours vectoriels |
| `src/fonts.rs` | Google Fonts download + cache |
| `src/brush.rs` | Brosses bitmap + canvas RGBA |
| `src/simulator.rs` | Simulation pression + tamponnage |
| `src/server.rs` | Serveur TCP + file d'attente |
| `src/recorder.rs` | Export/import JSON |
| `README.md` | Documentation utilisateur |
| `.github/workflows/ci.yml` | Pipeline CI GitHub Actions |
| `scripts/package.ps1` | Script de packaging reproductible |
| `scripts/send_job.py` | Script Python d'envoi de jobs TCP |
| `assets/fonts/` | Cache TTF téléchargés |
| `ordre_de_mission/PLAN.md` | Ce fichier |

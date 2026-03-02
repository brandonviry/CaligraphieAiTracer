# CaligraphieAiTracer — Contexte de reprise

## Ce que fait ce projet

Application Rust qui simule un calligraphe en train d'écrire automatiquement.
L'utilisateur (ou un script externe) donne un texte, une police, une brosse, une couleur
→ l'application trace lettre par lettre avec pression et inclinaison simulées.
**Aucun dessin manuel par l'humain.**

---

## Etat du projet (v0.1.0)

Projet complet et fonctionnel. CI + CD configurés sur GitHub.

- Repo : `https://github.com/brandonviry/CaligraphieAiTracer`
- Branch principale : `main`
- Lancer : `cargo run --release`
- Tester : `cargo test --all`
- Packager : `.\scripts\package.ps1`
- Publier une release : `git tag v0.X.0 && git push --tags`

---

## Stack technique

| Quoi | Choix |
|---|---|
| Langage | Rust 2021 |
| GUI | eframe 0.33 / egui 0.33 (immediate-mode) |
| Rendu police | fontdue 0.9 + ttf-parser 0.25 |
| Sérialisation | serde + serde_json 1.0 |
| Config | toml 0.8 (fichier `config.toml` externe) |
| Réseau | ureq 2 (téléchargement polices) |
| Images | image 0.25 (brosses PNG) |

---

## Fichiers importants

```
CaligraphieAiTracer/
├── src/
│   ├── main.rs          Point d'entrée, init eframe
│   ├── app.rs           UI egui + moteur d'animation (fichier principal)
│   ├── stroke.rs        Modèle de données : StrokePoint, Stroke
│   ├── glyph.rs         Extraction contours vectoriels depuis TTF
│   ├── fonts.rs         Téléchargement Google Fonts + cache local
│   ├── brush.rs         4 types de brosses bitmap, canvas RGBA
│   ├── simulator.rs     Simulation pression gaussienne + tamponnage
│   ├── server.rs        Serveur TCP localhost:7777 + file de jobs
│   └── recorder.rs      Export / Import JSON du tracé
├── scripts/
│   ├── send_job.py      Script Python pour envoyer des jobs TCP
│   └── package.ps1      Script de packaging reproductible (Windows)
├── .github/workflows/
│   ├── ci.yml           Tests + build sur chaque push
│   └── cd.yml           Release GitHub sur git tag v*
├── config.toml          Configuration (canvas, brosse, polices...)
├── Cargo.toml           Dépendances (version du projet ici)
└── README.md            Documentation utilisateur complète
```

---

## Architecture en 2 minutes

```
Script externe (Python, etc.)
        │  JSON sur localhost:7777
        ▼
Thread TCP (server.rs)
        │  Arc<Mutex<VecDeque<DrawJob>>>
        ▼
Thread UI (app.rs / egui)
        │
        ├─ fonts.rs    → charge / télécharge la police TTF
        ├─ glyph.rs    → TTF → courbes de Bézier → polylines
        ├─ simulator.rs → polylines + pression gaussienne → StrokePoints
        ├─ brush.rs    → StrokePoints → pixels sur canvas RGBA
        └─ recorder.rs → export/import JSON du tracé complet
```

Le thread UI tourne en boucle egui. À chaque frame :
1. Il dépile un job si l'animation précédente est terminée
2. Il avance l'animation en cours d'un certain nombre de points (selon `speed`)
3. Il repeint le canvas egui depuis le buffer RGBA interne

---

## Données clés

**DrawJob** (ce qu'un script envoie) :
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
Tous les champs sauf `text` sont optionnels.

**Types de brosses :**
- `round_smooth` — cercle gaussien
- `flat_calligraphy` — rectangle incliné 45°
- `brush_pen` — ellipse avec grain
- `dry_ink` — texture arrachée

**Polices disponibles (configurables dans config.toml) :**
Dancing Script, Great Vibes, Pacifico, Sacramento, Caveat, Pinyon Script, Alex Brush, Allura

---

## Ce qui n'est PAS fait (pistes futures)

- Export image (PNG/SVG) du tracé final
- Sélection de la couleur de fond du canvas
- Support multi-lignes (retour à la ligne automatique)
- Brosses personnalisées depuis l'UI (actuellement : PNG déposé dans `assets/brushes/`)
- Historique des tracés dans l'UI
- Port TCP configurable depuis l'UI (actuellement : uniquement via config.toml)

---

## Workflow de release

```bash
# Développement normal
git add ...
git commit -m "..."
git push
# → CI lance tests + build (artifact temporaire sur GitHub Actions)

# Publier une version
git tag v0.2.0
git push --tags
# → CD crée une Release GitHub avec ZIP téléchargeable publiquement
```

La version dans le tag doit correspondre à la version dans `Cargo.toml`.

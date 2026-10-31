# ShotCraft Engine

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Français](README.fr.md)

## 1. Fonctionnalités

### Origine et modifications

**ShotCraft Engine est un projet dérivé du code source de [mmartinez/shotcraft](https://github.com/mmartinez/shotcraft), modifié et réduit.** Il conserve du code de capture et d’assemblage ainsi que les mentions de copyright des contributeurs de ShotCraft. Il est maintenu indépendamment et ne constitue pas une publication officielle du projet d’origine.

Par rapport à l’application d’origine, ce projet :

- Extrait la capture dans une seule bibliothèque Rust et une CLI ; retire de la compilation active l’interface egui, l’éditeur, l’OCR, la zone de notification, l’historique, les réglages applicatifs, l’envoi, MCP et les ressources de l’application.
- Revoit la correspondance des images avec des blocs texturés et une vérification plus large des pixels, conserve les en-têtes et pieds fixes, actualise le chevauchement et garde la dernière image acceptée comme référence d’assemblage. Les idées algorithmiques de [LiteSnap](https://github.com/HuibingLin/LiteSnap) ont inspiré ces changements ; aucun code produit, binaire ou ressource LiteSnap n’est inclus. Voir [ATTRIBUTION.md](ATTRIBUTION.md).
- Sélectionne l’écran à partir du rectangle physique et de l’échelle de la fenêtre hôte, traite les coordonnées à DPI mixtes et initialise la gestion DPI dans la CLI Windows. Le chemin de capture pixmap X11 Linux conservé provient du projet d’origine. Les limites restantes sont décrites ci-dessous.
- Fournit des API JPEG/PNG vers Base64, une qualité JPEG configurable en Rust et des listes d’images ordonnées, sans panneau de capture ni modification du presse-papiers.
- Ajoute l’arrêt coopératif par drapeau atomique Rust et commande `stop` sur stdin, en conservant les captures acquises.
- Ajoute la capture simple `--single` et les captures PageDown séparées `--pages`, en plus de l’assemblage défilant.
- Remplace le packaging de l’application par des compilations manuelles du composant pour macOS ARM64/Intel, Windows x64 et Ubuntu x64, avec une documentation d’intégration en quatre langues.

### Modes de capture

Composant Rust de capture d’écran complet, intégrable dans d’autres logiciels, avec un exécutable autonome `scroll_capture`.

| Mode | Entrées envoyées | Résultat |
| --- | --- | --- |
| Par défaut / `CaptureMode::Stitched` | Déplacement du pointeur au centre de l’écran choisi, puis défilement vers le bas | Une image assemblée |
| `--single` / `CaptureMode::Single` | Aucune | Une capture du contenu actuel de l’écran |
| `--pages` / `CaptureMode::Pages` | PageDown | Des captures complètes séparées, dans l’ordre |

La sélection de l’écran utilise le rectangle extérieur de la fenêtre appelante. La bibliothèque produit du JPEG ou du PNG en Base64, sans redimensionnement. Elle ne crée aucune fenêtre, n’écrit aucun fichier et ne modifie pas le presse-papiers ; la CLI enregistre les résultats. Un arrêt coopératif conserve le contenu acquis. Le mode simple a été validé sur un bureau réel, ce qui ne constitue pas une validation sur toutes les plateformes.

## 2. Utilisation détaillée

### Syntaxe et paramètres

Les crochets indiquent des éléments facultatifs et ne doivent pas être saisis :

```text
scroll_capture [--single | --pages] <new-output-directory> [x y width height scale_factor]
scroll_capture --help
scroll_capture -h
```

| Paramètre | Valeurs acceptées | Valeur par défaut / condition |
| --- | --- | --- |
| `--single` | Option placée avant le répertoire ; incompatible avec `--pages` | Sans option de mode : défilement avec assemblage |
| `--pages` | Option placée avant le répertoire ; incompatible avec `--single` | Sans option de mode : défilement avec assemblage |
| `new-output-directory` | Répertoire inexistant dont le parent existe ; entourer de guillemets les chemins avec espaces | Obligatoire, aucun écrasement |
| `x`, `y` | Entiers `i32`, de −2,147,483,648 à 2,147,483,647 ; coordonnées physiques du coin supérieur gauche de la fenêtre | `0`, `0` |
| `width`, `height` | Entiers `u32`, de 1 à 4,294,967,295 ; dimensions physiques extérieures de la fenêtre | `100`, `100` |
| `scale_factor` | `f64` fini et strictement positif ; facteur propre à la fenêtre, utilisé pour convertir les coordonnées macOS | `1.0` |
| `--help`, `-h` | Seul, ou juste après `--single` / `--pages` | Aide uniquement, sans capture ni création de répertoire |

Fournir les cinq valeurs géométriques dans l’ordre ou les omettre toutes. Elles sélectionnent un écran, pas une zone de recadrage : l’écran ayant le plus grand chevauchement est choisi, ou celui dont le centre est le plus proche en l’absence de chevauchement. Sans géométrie, `(0, 0, 100, 100, 1.0)` sélectionne près de l’origine du bureau, généralement l’écran principal. La CLI ne lit pas automatiquement la position du terminal ou de Pluely14. Pour un répertoire commençant par `-`, utiliser un chemin tel que `./--name`.

La CLI utilise toujours **JPEG qualité 80** et un **délai de préparation de 10 secondes**. Les options `--format`, `--quality` et `--delay` n’existent pas ; utiliser l’API Rust pour choisir l’encodage.

### Exemples

Après compilation, sous macOS / Ubuntu :

```sh
./target/release/scroll_capture ./capture-01
./target/release/scroll_capture --single ./single-01
./target/release/scroll_capture --pages ./pages-01
```

Windows PowerShell :

```powershell
.\target\release\scroll_capture.exe .\capture-01
.\target\release\scroll_capture.exe --single .\single-01
.\target\release\scroll_capture.exe --pages .\pages-01
```

Avec Cargo, les arguments du programme suivent `--`. Le deuxième exemple fournit le rectangle physique d’une fenêtre sur un écran Retina :

```sh
cargo run --release --locked --bin scroll_capture -- --single ./single-01
./target/release/scroll_capture --pages ./pages-retina-01 200 200 800 600 2
```

Utiliser un nouveau répertoire différent pour chaque commande. Pendant la préparation, afficher le contenu cible ; commencer le défilement à la position souhaitée. Le mode simple capture la vue actuelle sans revenir en haut de la page.

Le mode pages capture d’abord une image, puis envoie un PageDown et attend 350 ms avant chaque capture suivante. Donner soi-même le focus au corps de la page sur l’écran sélectionné. Ce mode ne déplace pas le pointeur, ne clique pas et ne prend pas le focus. La distance dépend de l’application, pas exactement de la hauteur de l’écran ; les chevauchements sont acceptés. La barre d’adresse, un éditeur ou une zone de défilement imbriquée peut recevoir la touche.

### Fichiers et arrêt

| Mode | Fichiers produits |
| --- | --- |
| Assemblé / Simple | `capture.jpg`, `capture.base64` |
| Pages | `page-0001.jpg`, `page-0001.base64`, `page-0002.jpg`, `page-0002.base64`, … |

Chaque fichier `.base64` contient le Base64 standard du JPEG correspondant, sans préfixe data URI. Les captures complètes gardent leurs dimensions d’origine ; l’image assemblée garde les dimensions de l’assemblage. Aucun redimensionnement n’est effectué.

Saisir exactement `stop` en minuscules, sans espaces, puis Entrée sur l’entrée standard pour arrêter le défilement et conserver le contenu acquis. Avant la première image, l’arrêt renvoie une erreur sans image. Le mode simple conserve une image déjà capturée. Revenir au terminal peut contaminer la capture ; l’hôte peut envoyer `stop
` par un tube sans changer le focus.

Les fichiers sont écrits après le retour de la fonction, pas à chaque image. `Ctrl+C` ou un arrêt forcé peut perdre tous les résultats non enregistrés. Une erreur de capture ordinaire n’enregistre généralement pas les images précédentes ; une erreur d’écriture peut laisser des fichiers partiels. Le code de sortie est 0 en cas de réussite et non nul en cas d’erreur. stdout affiche les chemins, dimensions et tailles JPEG en octets, pas le Base64 brut. L’existence du répertoire ne suffit pas à prouver la réussite.

## 3. Appel depuis un autre logiciel

### Dépendance Rust et API

Pour un dépôt local, adapter le chemin :

```toml
[dependencies]
shotcraft-engine = { path = "../shotcraft-engine" }
```

Ou utiliser Git ; un dépôt privé nécessite un accès Git configuré dans l’environnement de compilation :

```toml
[dependencies]
shotcraft-engine = { git = "https://github.com/teddyxiong/shotcraft-engine.git", branch = "main" }
```

Cargo compile la bibliothèque dans l’hôte. Il ne s’agit pas d’une DLL avec ABI C ni d’un service HTTP prêt à utiliser. Les points d’entrée publics sont dans `shotcraft_engine::scrolling` :

```rust,ignore
pub fn capture_to_base64(
    bounds: WindowBounds,
    format: Option<String>,
    quality: Option<u8>,
    stop: &std::sync::atomic::AtomicBool,
    mode: CaptureMode,
) -> shotcraft_engine::Result<Vec<String>>;

pub fn encode_image_to_base64(
    image: &image::RgbaImage,
    format: Option<String>,
    quality: Option<u8>,
) -> shotcraft_engine::Result<String>;
```

| Paramètre API | Valeurs et comportement |
| --- | --- |
| `bounds: WindowBounds` | Champs obligatoires : `x: i32`, `y: i32`, `width: u32`, `height: u32`, `scale_factor: f64`. Mêmes unités, plages et règles de sélection que la CLI ; aucune géométrie par défaut dans la bibliothèque |
| `format: Option<String>` | Uniquement `Some("jpg".into())` ou `Some("jpeg".into())` en minuscules produit du JPEG ; `None`, `"png"` et toute autre chaîne produisent du PNG. Sensible à la casse |
| `quality: Option<u8>` | Entrée de 0 à 255 ; `None` vaut 80 ; ramenée à 1–100 pour JPEG. Ignorée pour PNG. La transparence JPEG est composée sur fond blanc |
| `stop: &AtomicBool` | Obligatoire ; initialiser à `false`, puis écrire `true` depuis un autre thread ou gestionnaire d’événement pour demander l’arrêt |
| `mode: CaptureMode` | Obligatoire : `Single`, `Stitched` ou `Pages`, sans valeur par défaut |
| `image: &image::RgbaImage` | Encodeur uniquement : image RGBA existante, sans capture ni entrée |

Exemple de capture simple dans un thread de travail. Remplacer les valeurs par la position et les dimensions physiques extérieures de la fenêtre hôte et son propre facteur d’échelle, par exemple `outer_position`, `outer_size`, `scale_factor` de Tauri :

```rust
use shotcraft_engine::scrolling::{CaptureMode, WindowBounds, capture_to_base64};
use std::sync::{Arc, atomic::AtomicBool};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bounds = WindowBounds {
        x: 200, y: 200, width: 800, height: 600, scale_factor: 2.0,
    };
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let task = std::thread::spawn(move || {
        capture_to_base64(
            bounds, Some("jpeg".into()), Some(80), &worker_stop,
            CaptureMode::Single,
        )
    });
    let images = task.join()
        .map_err(|_| std::io::Error::other("capture worker panicked"))??;
    println!("Captured images: {}", images.len());
    Ok(())
}
```

Choisir `CaptureMode::Stitched` pour une image longue ou `CaptureMode::Pages` pour des pages séparées. `Single` et `Stitched` renvoient une liste de longueur 1 ; `Pages` renvoie une chaîne par image retenue. Un résultat réussi contient au moins une image, renvoyée avec les autres à la fin de l’appel, sans rappel par image.

L’API est synchrone et bloquante, sans le délai CLI de 10 secondes. Utiliser un thread de travail ou le `spawn_blocking` de l’hôte ; ne pas attendre un `join` synchrone dans le thread d’événements de l’interface. L’hôte gère visibilité, focus, permissions et raccourcis. La bibliothèque ne dépend pas de Tauri et ne fournit pas de raccourci global d’arrêt.

Conserver l’`Arc<AtomicBool>` dans l’état de l’application. Dans la commande d’arrêt ou le gestionnaire de raccourci :

```rust,ignore
stop.store(true, std::sync::atomic::Ordering::Relaxed);
```

Créer un nouveau drapeau pour chaque tâche, sans le réinitialiser pendant la capture. L’arrêt conserve les images validées. La capture, l’assemblage, l’encodage en cours et le reste de l’attente de 350 ms ne sont pas interrompus de force ; les entrées déjà envoyées ne sont pas annulables. Le mode simple ignore cette attente, toutes les entrées, le backend continu et l’assembleur.

Encoder directement une image existante :

```rust,ignore
let base64 = shotcraft_engine::scrolling::encode_image_to_base64(
    &image, Some("jpeg".into()), Some(80),
)?;
```

Le décodage Base64 donne les octets du format choisi. Transmettre la chaîne directement si le destinataire attend du Base64 simple ; sinon ajouter `data:image/jpeg;base64,` ou `data:image/png;base64,`. En mode Pages, transmettre les images séparément dans l’ordre ; concaténer des chaînes Base64 ne crée pas une image. Aucun envoi automatique vers un service d’IA ou Pluely14 n’est réalisé.

### CLI comme sous-processus

Distribuer l’exécutable correspondant au système et à l’architecture, avec les licences requises. Le lancer avec les arguments ci-dessus et un nouveau répertoire, en conservant stdin dans un tube. Pour arrêter, écrire `stop
` puis vider le tampon ; fermer stdin seul ne suffit pas. Attendre le code de sortie 0 avant de lire les fichiers `.jpg` / `.base64`. La CLI est un exécutable autonome ; l’intégration Rust directe utilise la dépendance source décrite ci-dessus.

## 4. Plateformes et limites

### Comportement actuel

- Assemblé : au plus 50 tentatives de capture ou 40 000 pixels de hauteur. Trois images consécutives inchangées terminent la capture ; trois échecs consécutifs de correspondance produisent une erreur. À la limite, seule la partie acquise est renvoyée, sans garantie d’atteindre le bas. Chaque étape utilise 3 unités verticales de molette, puis attend 350 ms.
- Pages : au plus 50 tentatives. Les images RGBA consécutives exactement identiques sont ignorées ; trois images inchangées terminent la capture. Pas de dédoublonnage par similarité, de correspondance obligatoire ni de limite d’assemblage de 40 000 pixels. Vidéo, animations ou changements d’état système peuvent empêcher la détection de fin : utiliser `stop` ou la limite de tentatives.
- Simple : une capture sans entrée ni assemblage. L’autorisation de capture reste nécessaire ; cela ne prouve pas la prise en charge native de Wayland.
- Si une erreur de capture ultérieure ou d’entrée coïncide avec l’arrêt, le contenu validé est prioritaire. Sans fenêtre propre au composant, les autres contenus visibles restent capturables ; les modes de défilement modifient réellement le bureau.
- macOS : le processus appelant doit avoir l’autorisation d’enregistrement de l’écran et, pour les modes de défilement, les permissions nécessaires aux entrées automatisées.
- Windows : la CLI initialise la gestion DPI ; la bibliothèque ne modifie pas celle du processus hôte. Le positionnement Enigo en mode assemblé utilise la normalisation de l’écran principal, rendant la molette sur écran secondaire connue comme peu fiable. Valider d’abord sur le principal. Pages envoie les touches à l’application ayant le focus ; les écrans secondaires nécessitent encore des essais réels.
- Ubuntu : l’entrée de défilement utilise X11. Vérifier avec `echo "$XDG_SESSION_TYPE"` ; le défilement Wayland natif n’est pas adapté. Compilation sous Ubuntu 24.04 ; `--help` dans un conteneur 22.04 vérifie seulement le démarrage.

Bibliothèques d’exécution Ubuntu :

```sh
sudo apt-get install -y libpipewire-0.3-0 libxcb1 libgbm1 libegl1 libx11-6 libwayland-client0
```

### Wayland natif : conditions et état actuel

La version verrouillée de xcap dispose de chemins de capture ponctuelle Wayland, mais le composant ne maintient pas de session Portal/PipeWire continue. Enigo utilise actuellement son backend X11 par défaut. Installer les en-têtes, compiler ou réussir une capture ne prouve pas le défilement automatique sous Wayland natif.

| Capacité nécessaire | Rôle |
| --- | --- |
| Bureau Wayland réel et D-Bus de session utilisateur | Communiquer avec les services du bureau et tester réellement |
| xdg-desktop-portal et backend correspondant au bureau | GNOME/KDE/etc. doivent fournir les interfaces requises |
| ScreenCast Portal et PipeWire | Recevoir les images continues de l’écran choisi dans une session autorisée |
| Entrées RemoteDesktop Portal | Pointeur/molette pour l’assemblage, clavier PageDown pour Pages, via méthodes compatibles ou libei/EIS |
| Autorisations écran et périphériques d’entrée concernés | L’autorisation écran ne remplace pas celle du pointeur ou du clavier |

L’adaptation consiste à réutiliser une session autorisée de capture/entrée, faire correspondre position, dimensions et échelle du flux à l’écran cible, puis reprendre assemblage, encodage et arrêt. Activer une seule feature Enigo ne suffit pas. Les coordonnées du compositeur ne sont pas nécessairement des pixels physiques ; la sélection actuelle par chevauchement doit aussi être validée.

Le composant peut éviter ses propres panneaux, mais le parcours Portal standard présente généralement une autorisation système et un choix d’écran. Selon le bureau, `persist_mode` et `restore_token` réduisent les demandes répétées ; les jetons doivent être actualisés et révocation, changement d’écran ou échec de restauration peuvent provoquer une nouvelle demande. Aucune garantie générale de premier usage silencieux ou d’absence permanente de dialogue système n’est possible.

Pour arrêter par raccourci global, l’hôte doit aussi vérifier et utiliser GlobalShortcuts Portal. Valider d’abord flux continu, entrées, assemblage et arrêt partiel sur Ubuntu 24.04 + GNOME Wayland, puis sur KDE et les autres bureaux. La version d’Ubuntu seule ne détermine pas la compatibilité. Le mode simple dépend toujours du backend de capture existant et des interactions d’autorisation du bureau.

Références Portal : [ScreenCast](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html) · [RemoteDesktop](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html) · [GlobalShortcuts](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)

## 5. Compilation locale et sur GitHub

### Prérequis et compilation locale

Installer **Rust 1.95 ou ultérieur** et Cargo. Récupérer les sources et vérifier la chaîne d’outils :

```sh
git clone https://github.com/teddyxiong/shotcraft-engine.git
cd shotcraft-engine
rustc --version
cargo --version
```

Avec rustup, `rustup update stable` met à jour la version stable. Configurer au préalable l’accès aux dépôts privés.

| Plateforme | Prérequis |
| --- | --- |
| macOS | Xcode Command Line Tools (`xcode-select --install`) et chaîne Rust native |
| Windows x64 | Visual Studio C++ Build Tools, Windows SDK et chaîne Rust `x86_64-pc-windows-msvc` ; utiliser un terminal développeur configuré |
| Ubuntu 24.04 x64 | Rust et les paquets de développement suivants |

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libclang-dev libxcb1-dev libgbm-dev libegl-dev libpipewire-0.3-dev libspa-0.2-dev libwayland-dev
```

Depuis la racine du dépôt, compiler la CLI ou vérifier la bibliothèque :

```sh
cargo build --release --locked --bin scroll_capture
cargo check --lib --locked
```

L’exécutable natif est `target/release/scroll_capture` (macOS/Ubuntu) ou `target/release/scroll_capture.exe` (Windows). Cargo compile automatiquement la bibliothèque lorsqu’un autre projet Rust en dépend. Avec `--target <triple>`, la sortie est dans `target/<triple>/release/`.

Vérifications du projet :

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

### Compilation manuelle GitHub Actions

Seul **Engine native builds**, dans [engine-build.yml](.github/workflows/engine-build.yml), est actif. Le déclencheur est uniquement `workflow_dispatch` : push, PR, tags et planifications ne lancent pas de compilation.

1. Ouvrir [Actions](https://github.com/teddyxiong/shotcraft-engine/actions/workflows/engine-build.yml).
2. Cliquer sur **Run workflow**, sélectionner la branche contenant les changements souhaités, puis confirmer avec **Run workflow**.
3. Attendre les quatre jobs et télécharger l’Artifact correspondant depuis la page d’exécution. Conservation : 14 jours.

| Plateforme | Triple cible | Artifact |
| --- | --- | --- |
| macOS ARM64 | `aarch64-apple-darwin` | `scroll-capture-macos-arm64` |
| macOS Intel | `x86_64-apple-darwin` | `scroll-capture-macos-x64` |
| Windows x64 | `x86_64-pc-windows-msvc` | `scroll-capture-windows-x64` |
| Ubuntu x64 | `x86_64-unknown-linux-gnu` | `scroll-capture-ubuntu-x64` |

Les jobs exécutent formatage, Clippy, tests, compilation release et `--help` ; Ubuntu ajoute le contrôle de démarrage 22.04. Les Artifacts contiennent l’exécutable, le README anglais, les licences, les attributions, le commit et la chaîne d’outils. Les autres langues sont disponibles dans le dépôt.

Extraire l’Artifact correspondant. Dans son répertoire, lancer `./scroll_capture --single ./single-01` ou, sous Windows, `.\scroll_capture.exe --single .\single-01`. Sous macOS/Ubuntu, exécuter auparavant `chmod +x ./scroll_capture` si nécessaire. Pour tester le défilement, reprendre les arguments de la section 2 et un nouveau répertoire à chaque fois. Vérifier capture complète et arrêt sur un vrai bureau ; la compilation et l’aide en CI ne valident pas le comportement réel.

## 6. Sources et licence

### Licence et autorisations

Les contributions et modifications de cette révision que les mainteneurs ont le droit de licencier relèvent de la licence personnalisée [ShotCraft Engine Source-Available License 1.0](LICENSE). Cette révision entière n’est plus proposée globalement sous MIT/Apache. Le texte anglais de `LICENSE` fait foi ; cette section en résume la portée.

| Usage | Autorisation |
| --- | --- |
| Logiciel publié par une personne, une entreprise ou une organisation, sans frais d’accès, d’utilisation ou de fonctionnalité pour ses utilisateurs | Gratuit selon `LICENSE` |
| Logiciel gratuit financé par publicité, parrainage volontaire ou dons facultatifs | Gratuit si aucun paiement n’est requis pour l’accès, l’usage ou les fonctionnalités |
| Usage interne propre à une organisation, y compris ses activités commerciales | Gratuit selon `LICENSE` |
| Logiciel facturant achat, abonnement, licence, offre payante, déblocage de fonction ou accès à un logiciel hébergé | Licence commerciale payante écrite distincte obligatoire, même si la capture elle-même est gratuite |

La règle couvre l’intégration de la bibliothèque, l’appel d’une CLI intégrée et les versions modifiées. Cette licence n’exige pas la publication du code de l’application hôte ou des modifications privées. Contacter le mainteneur via une [Issue du dépôt](https://github.com/teddyxiong/shotcraft-engine/issues) pour convenir séparément par écrit du prix et du périmètre. Une demande ou un don seul ne vaut pas autorisation.

Les portions ShotCraft d’origine conservent **MIT OR Apache-2.0** et leurs copyrights ; les dépendances gardent leurs propres licences. Les nouvelles conditions couvrent uniquement ce que les mainteneurs peuvent licencier et n’annulent pas les droits indépendants acquis auprès du projet d’origine ou précédemment. En particulier, **`v0.1.1` au commit `114184a9a7a37928c818b1aa8b804bcf02c869a7` et les versions antérieures gardent leurs conditions initiales**. Un titulaire d’une autorisation permissive antérieure valide peut continuer à utiliser l’ancien code concerné selon celle-ci ; cette modification ne force pas les titulaires existants à payer.

Il s’agit d’une licence **source-available** avec restriction sur les logiciels payants, et non d’une licence open source approuvée par l’OSI. Le tag `v0.1.1` reste inchangé ; le numéro de paquet `0.1.1` seul n’identifie pas les conditions des commits ultérieurs de main. Fixer et examiner le commit exact. Aucun nouveau tag de publication n’est créé.

Si vous publiez des modifications ou un dérivé, une notification par Issue est appréciée. Elle est volontaire, ne constitue pas une condition supplémentaire et ne remplace pas une autorisation payante requise.

| Fichier | Responsabilité |
| --- | --- |
| `src/lib.rs` | Exports publics et erreurs structurées |
| `src/capture.rs` | Capture d’écran, y compris le chemin pixmap X11 Linux |
| `src/stitch.rs` | Assemblage des images de défilement à partir des pixels |
| `src/scrolling.rs` | Sélection d’écran, modes, arrêt et encodage Base64 |
| `src/bin/scroll_capture.rs` | Arguments CLI et fichiers de sortie |

Les attributions et textes d’origine sont conservés dans [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE), [NOTICE](NOTICE) et [ATTRIBUTION.md](ATTRIBUTION.md). Les licences des dépendances figurent dans le fichier généré [THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md). Distribuer [LICENSE](LICENSE) avec toutes les mentions applicables d’origine et des dépendances lors de la redistribution ou de l’intégration. Le code ShotCraft d’origine n’est pas relicencié par ce document.

L’interface, l’OCR, l’éditeur, la zone de notification et les ressources de l’application d’origine sont exclus du composant actif. Le répertoire local ignoré `backups/` sert à la restauration et n’est inclus ni dans la distribution Git ni dans la compilation.

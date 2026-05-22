# 나이테 (naite)

나이테는 커밋이 쌓여 만드는 히스토리 레이어를 나무의 나이테처럼 읽기
쉽게 보여주는 네이티브 데스크톱 Git 클라이언트입니다. Rust와 iced로
만든 로컬 우선 macOS 앱입니다.

## 사용 전제

현재 버전의 나이테는 사용자의 로컬 개발 환경을 그대로 사용합니다. 앱이
Git 인증 정보나 GitHub 토큰을 직접 설정하거나 보관하지 않습니다.

- private repository clone, fetch, pull, push, release promotion처럼 원격과
  통신하는 Git 작업은 사용자의 shell 환경에서 `git` 인증이 이미 동작해야
  합니다. 예를 들어 SSH key, credential helper, keychain, access token
  설정이 먼저 완료되어 있어야 합니다.
- GitHub PR/issue 기능은 GitHub CLI인 `gh`가 설치되어 있고 인증되어 있어야
  사용할 수 있습니다. 먼저 `gh auth login`으로 로그인하고, 필요하면
  `gh auth status`로 현재 인증 상태를 확인하세요.

## 나이테만의 특수 기능

나이테는 모든 Git GUI를 그대로 복제하려는 도구가 아닙니다. 레이어처럼
쌓이는 히스토리를 읽고, 로컬 작업 상태를 숨기지 않으며, 위험한 Git 작업을
실행 전에 확인할 수 있게 만드는 데 집중합니다.

- **WIP row가 있는 레이어드 히스토리:** commit list, graph lane, refs,
  branch sync chip, 현재 worktree 상태를 한 화면에서 함께 봅니다. 아직
  commit하지 않은 WIP row도 commit과 같은 diff detail 화면으로 열립니다.
- **Hunk 중심 diff 검토:** WIP와 commit diff에서 파일 선택, hunk 이동,
  syntax highlight, unified/focused-hunk/inline/split view를 지원합니다. 파일
  또는 텍스트 hunk 단위 stage, unstage, discard가 검토 중인 hunk 가까이에
  배치됩니다.
- **시각적인 히스토리 수술:** merge, rebase, conflict resolution, reword,
  drop, squash, fixup, edit, reorder, undo, redo는 대상 commit/ref와 실행될
  Git 명령의 형태를 prompt에서 확인한 뒤 실행합니다.
- **Interactive rebase planner:** rebase 대상을 고르면 action chip, row
  reorder, commit별 diff detail, reword draft, Keep Mine/Squash Mine/Squash All
  preset이 있는 todo-list 스타일 planner가 열립니다. 계획은 로컬 rebase만
  적용하거나 rebase 후 force push까지 이어갈 수 있습니다.
- **Release Promotion:** `staging -> main` 같은 source/target branch 후보를
  감지하고 remote ref를 fetch한 뒤 양쪽 branch를 remote 기준으로
  force-sync합니다. 필요하면 안전 backup branch를 만들고 rebase planner를
  연 다음, target update, target push, source를 target에서 다시
  `--force-with-lease`로 sync하는 후속 action을 제공합니다.
- **PR, issue, worktree handoff:** GitHub CLI 기반 패널에서 PR을 list, filter,
  search, create, open, checkout할 수 있고 새 worktree로 checkout할 수도
  있습니다. PR row는 CI, review, label, reviewer, draft, merge state,
  linked issue metadata를 보여주며, issue row는 open/assigned/mentioned/
  closed/search mode로 필터링할 수 있습니다.
- **Local workspace cockpit:** 최근 저장소, 즐겨찾기, repo tab, workspace
  dashboard로 여러 로컬 저장소를 관리합니다. dashboard는 dirty 여부,
  ahead/behind count, worktree count, last fetch age를 요약하고 multi-repo
  fetch/pull/open/locate/remove를 지원합니다.
- **Repo-scoped terminal panel:** terminal session은 global shell이 아니라
  활성 repository 또는 worktree에 붙고, 실행 상태와 shell context를 앱
  안에서 추적합니다.
- **Local-first provider boundary:** Git 읽기/쓰기는 `naite-core`에 있고,
  provider 기능은 사용자의 기존 `gh` 설정을 통합니다. cloud sync,
  telemetry, token storage, server-side source upload는 추가하지 않습니다.

## 터미널 기능

나이테의 터미널은 저장소 작업을 위해 앱 안에 붙어 있는 repo-scoped command
panel입니다. 독립적인 시스템 터미널을 완전히 대체하기보다, 현재 보고 있는
repository나 worktree에서 바로 명령을 실행하고 결과 맥락을 이어가기 위한
기능입니다.

- 활성 repository 또는 선택한 worktree의 경로를 기준으로 terminal session을
  엽니다.
- 여러 session을 만들고 전환하며, session별 label, shell, cwd, 실행 상태,
  마지막 명령, 마지막 exit code를 유지합니다.
- 실행 중인 명령에 interrupt/kill/close를 보낼 수 있고, 종료된 session은
  다시 시작할 수 있습니다.
- zsh integration을 통해 shell cwd, command history, command lifecycle
  event를 받아 앱 상태와 동기화합니다.
- 입력 중인 command에 대해 zsh history, session history, path completion,
  Git subcommand suggestion을 제공합니다.
- terminal session은 repository/worktree context에 묶이므로, PR checkout
  worktree나 workspace dashboard에서 열린 저장소의 후속 작업을 바로 이어갈
  수 있습니다.

## 기술 스택

- **UI:** [`iced`](https://crates.io/crates/iced) - retained-mode Rust GUI.
- **Git:** [`gix`](https://crates.io/crates/gix) - pure-Rust Git 구현.
- **파일 선택:** [`rfd`](https://crates.io/crates/rfd) - native folder/file dialog.
- **Async:** `tokio` - blocking Git 작업을 UI thread 밖에서 실행합니다.

## 프로젝트 구조

```
naite/
├── Cargo.toml                   ← workspace root
└── crates/
    ├── naite-core/              ← Git domain logic (gix 사용)
    │   └── src/                     repository read/write, diff/status parsing
    └── naite-app/               ← iced UI (naite-core 사용)
        └── src/                     app state, persistence, view, update
```

이 분리는 Git 로직을 UI dependency 없이 독립적으로 테스트하기 위한
경계입니다. UI crate는 `gix`를 직접 import하지 않습니다.

## 개발

```bash
cargo run -p naite-app          # debug
cargo run -p naite-app --release # release
scripts/macos-bundle.sh               # build target/debug/naite.app
scripts/macos-bundle.sh --release     # build target/release/naite.app
open target/debug/naite.app           # run with the project icon on macOS
```

## macOS 설치

설치 스크립트는 release build를 만들고 unsigned `naite.app` bundle을
생성한 뒤 `/Applications/naite.app`에 설치하고 앱을 엽니다. Cargo가 PATH에
없다면 먼저 Rust toolchain 경로를 추가합니다.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
scripts/macos-install.sh
```

자주 쓰는 옵션:

```bash
scripts/macos-install.sh --install-dir "$HOME/Applications" # 설치 위치 변경
scripts/macos-install.sh --no-open                          # 설치만 하고 실행하지 않음
scripts/macos-install.sh --no-pause                         # 터미널 종료 대기 생략
```

설치 로그는 `target/macos-install.log`에 남습니다. 현재 앱은 unsigned
bundle이므로 macOS 보안 경고가 뜰 수 있습니다.

## 저장소 자산

- GitHub social preview: `.github/social-preview.png`
- 업로드 위치: GitHub repository Settings → Social preview

# DailyWorkAlter

퇴근 30분 전에 알아서 떠서 오늘 한 일을 기록하게 하는 Windows 트레이 앱.
금요일(변경 가능)에는 그 주 전체를 펼쳐 보여주고 주간보고용으로 복사해준다.

트레이에 상주하다가 시간이 되면 창이 뜬다. 평소엔 아무것도 안 해도 된다.

## 빌드

```
npm install
npm run build
```

`src-tauri/target/release/dailyworkalter.exe` (약 5MB) 하나만 있으면 된다.
아무 폴더에 두고 한 번 실행하면 출퇴근 시간을 물어보고 트레이로 내려간다.
이후 부팅하면 자동으로 뜬다.

실행 경로에 따라 동작이 다르다:

- **최초 실행** — 온보딩
- **로그인 시 자동 실행** — 창 없이 조용히 트레이로 (`--autostart` 인자로 구분)
- **직접 실행** — 창을 띄운다. 트레이 아이콘이 숨겨져 있으면 실행해도 아무 반응이
  없는 것처럼 보이기 때문
- **이미 떠 있는데 또 실행** — 새 프로세스는 종료되고 기존 인스턴스가 창을 띄운다

> 자동 실행은 실행된 위치를 기억한다. exe를 옮기면 옮긴 자리에서 한 번 실행해 경로를 갱신할 것.

## 조작

| 키 | 동작 |
|---|---|
| `Enter` | 새 항목 (현재 들여쓰기 유지) |
| `Tab` / `Shift+Tab` | 들여쓰기 / 내어쓰기 (한 단계까지) |
| `Ctrl+Enter`, `Esc` | 저장하고 닫기 |
| `Ctrl+Alt+D` | 아무 때나 창 열기 (전역) |

`- `는 자동으로 붙는다. 입력 중 자동 저장된다.
Windows 11은 새 트레이 아이콘을 숨기므로 시계 왼쪽 `^`에서 꺼내 고정하면 편하다.

## 데이터

```
%APPDATA%\DailyWorkAlter\
├─ config.json     설정
├─ state.json      앱 내부 상태 (건드릴 필요 없음)
└─ logs\2026-09-07.md
```

로컬 전용. 동기화·백업 없음 — 필요하면 폴더를 복사하면 된다.
로그는 그냥 마크다운이라 앱 없이도 읽고 검색할 수 있다.

설정은 트레이 → 설정에서 바꾸거나 `config.json`을 직접 고쳐도 된다
(직접 고치면 재시작 없이 반영되고, 문법을 깨뜨리면 이전 설정을 유지한 채 알림만 뜬다).
비자명한 값만:

- `work.workdays` — **1=월 … 7=일**
- `weekly.mode` — `fixedDay`(지정 요일) 또는 `lastWorkday`(그 주 마지막 근무일)
- `notify.catchUpHours` — 절전 등으로 놓쳤을 때 몇 시간까지 늦게라도 띄울지

지울 때는 exe, 위 폴더, 그리고
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`의 `DailyWorkAlter` 항목.

## 개발

```
npm run dev              개발 실행 (핫리로드)
npm run build            릴리스 exe
npm run kill             떠 있는 프로세스 종료
```

트레이 상주라 창을 닫아도 프로세스가 안 죽는다. 그 상태로 재빌드하면 exe가 잠겨
`os error 5`가 나므로 `npm run dev`/`build`가 먼저 정리한다.

디버그 빌드에는 트레이에 **테스트** 서브메뉴가 붙는다 — 온보딩·일일·주간 화면 강제 열기,
이번 주 샘플 로그 생성/삭제, 알림 상태 초기화. 17:30이나 금요일까지 기다리지 않고 확인하기 위한 것.

### 구조

```
src/                  프론트엔드 - 화면 전담
├─ main.js             뷰 라우팅 (?view= 쿼리로 진입)
├─ api.js              Rust 커맨드 래퍼
├─ views/              daily · weekly · settings · onboarding · header
└─ util/               date · markdown · bullet-editor

src-tauri/src/        Rust - 타이머·설정·파일 IO
├─ lib.rs              앱 조립
├─ commands.rs         JS ↔ Rust 경계
├─ scheduler.rs        60초 tick, 알림 판정
├─ devtools.rs         디버그 전용 테스트 헬퍼
├─ data/               디스크에 있는 것
│   └─ config · state · storage · watcher
└─ shell/              OS·Tauri 연동
    └─ window · tray · shortcut · autostart · notify
```

Rust 쪽은 무엇에 의존하는지로 나눴다 — 디스크(`data`), OS(`shell`), 그리고 어느 쪽도
아닌 순수 판단(`scheduler`). MVC로 나누지 않은 이유는 View가 프로세스 경계 너머 JS에
있고, `shell/`의 절반이 Model·View·Controller 어디에도 해당하지 않기 때문이다.

타이머가 JS가 아니라 Rust에 있는 이유는 창을 파기해서 메모리를 회수하기 때문이다.
창이 없는 동안엔 JS가 돌지 않으므로, 스케줄링을 JS에 두면 알림이 아예 뜨지 않는다.

아이콘은 `assets/app-icon.png`를 고친 뒤 `npx tauri icon assets/app-icon.png`.

설계 판단의 근거는 커밋 메시지에 적어뒀다.

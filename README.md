# Move Wallpapers

Windows 바탕화면 **아이콘 뒤(WorkerW 레이어)** 에 폴더 이미지 슬라이드쇼를 진짜
배경화면처럼 띄우는 앱. 시스템 배경화면 설정은 건드리지 않는다.

기존 Electron 버전을 **Rust + Tauri v2 + React** 로 전면 재작성한 결과물이다.

## 기술 스택

- **백엔드**: Rust + Tauri v2, `windows-sys` 로 Win32 (WorkerW 리페어런팅) 제어
- **프론트엔드**: React 18 + TypeScript + Vite (제어판 / 오버레이 / 영역 피커 3개 창)
- **런타임**: 시스템 WebView2 (별도 Chromium 번들 없음)

## 핵심 동작

- `SendMessage(Progman, 0x052C)` 로 WorkerW 생성 → `SHELLDLL_DefView` 뒤 WorkerW 탐색
- 각 오버레이 WebView2 창을 `SetParent` 로 WorkerW 자식으로 편입 → 아이콘 뒤 고정
- 모니터별 폴더 지정, 전체/영역(드래그) 선택, 셔플, 변경 주기, 트레이 상주, 설정 저장

## 개발

```bash
npm install
npm run tauri dev      # 개발 모드 (핫리로드)
npm run tauri build    # 릴리스 빌드 (NSIS 설치 파일)
```

Microsoft Store 제출용 MSIX는 릴리스 빌드 후 아래 스크립트로 만든다 (Windows SDK 필요).
결과물은 `src-tauri/target/msix/`에 생긴다.

```bash
powershell -ExecutionPolicy Bypass -File scripts/build-msix.ps1
```

아이콘 원본은 `src-tauri/icons/source.svg`(앱)와 `tray-source.svg`(트레이용 단순 버전)이다.
수정 후 다시 생성하려면:

```bash
npx tauri icon src-tauri/icons/source.svg
```

트레이 아이콘은 `tray-source.svg`를 같은 명령으로 임시 폴더(`-o`)에 뽑은 뒤
`32x32.png`를 `src-tauri/icons/tray.png`로 복사한다.

## 프로젝트 구조

```
src/                  React 프론트 (control / overlay / picker)
src-tauri/src/        Rust 백엔드
  win32.rs            WorkerW 리페어런팅 + DWM
  overlay.rs          오버레이/피커 창 생성 + 배치
  commands.rs         Tauri 커맨드 (구 IPC 대응)
  display.rs          모니터 열거
  config.rs           설정 로드/저장 (app_config_dir/config.json)
```

## 설정 파일

`%APPDATA%\com.movewallpapers.app\config.json`

```jsonc
{
  "intervalSec": 600,
  "shuffle": false,
  "monitors": {
    "\\\\.\\DISPLAY1": { "folder": "C:/Pictures", "region": null }
  }
}
```

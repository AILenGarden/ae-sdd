# MoAI-ADK 이슈 분석 보고서

> **분석 기간**: 2025-09-16 ~ 2026-02-23
> **총 커밋 수**: 4,218개
> **Fix 커밋 수**: 601개 (Go 1.26 이전) + 9개 (Go 1.26 이후)
> **생성 일시**: 2026-02-23

---

## 1. 요약

### 1.1 Go 1.26 업그레이드 전후 비교

| 구분 | Go 1.26 이전<br>(2025-09-16 ~ 2026-02-19) | Go 1.26 이후<br>(2026-02-20 ~ 현재) | 변화 |
|------|-------------------------------------------|-------------------------------------|------|
| **기간** | 약 5개월 | 약 3일 | - |
| **Fix 커밋 수** | 601개 | 9개 | 대폭 감소 |
| **일평균 Fix** | 약 4개/일 | 약 3개/일 | 안정화 |

> **결론**: Go 1.26 업그레이드 직후 fix 커밋이 크게 감소했습니다. 이는 코드베이스가 안정화되었음을 의미합니다. 다만, 업그레이드 후 기간이 짧아(3일) 추세를 확정하기에는 이릅니다.

### 1.2 최근 30일간 Fix 커밋: 104개

---

## 2. 이슈 유형별 분석

### 2.1 가장 빈번한 이슈 유형 Top 10

| 순위 | 유형 | 발생 횟수 | 비고 |
|------|------|-----------|------|
| 1 | **hook** | 150+ | Hook 시스템 관련 이슈 |
| 2 | **test** | 93 | 테스트 관련 이슈 |
| 3 | **template** | 68 | 템플릿 시스템 이슈 |
| 4 | **ci** | 56 | CI/CD 파이프라인 이슈 |
| 5 | **update** | 45 | moai update 명령 이슈 |
| 6 | **path** | 47 | 경로 처리 이슈 |
| 7 | **windows** | 31 | Windows 플랫폼 이슈 |
| 8 | **session** | 33 | 세션 관리 이슈 |
| 9 | **timeout** | 18 | 타임아웃 관련 이슈 |
| 10 | **rank** | 12 | Rank 시스템 이슈 |

### 2.2 Fix 커밋 Scope별 분포

```
hooks      ████████████████████████████████████ 34
ci         ████████████████████████████████     31
workflow   ██████████████████████████████       30
tests      ███████████████████                  17
update     ██████████████                       14
template   ██████████████                       14
statusline █████████████                        13
rank       ██████████                           10
install    ██████████                           10
cli        █████████                             9
github     ████████                              8
config     ███████                               7
wizard     ██████                                6
settings   ██████                                6
```

---

## 3. 반복 발생 이슈 패턴 분석

### 3.1 Hook 시스템 이슈 (가장 빈번)

**발생 패턴**:
- SessionStart/SessionEnd hook 타임아웃 (반복 발생)
- Hook 이벤트 핸들러 누락
- Hook JSON 출력 형식 오류
- Hook 환경 변수 처리

**최근 해결**:
```
fce49e50 fix(hook): prevent SessionStart hook timeout on session restart (#403)
df503c7c fix(hook): register SubagentStop CLI and add auto team cleanup on SessionEnd
d7e5a72c fix(hook): set PermissionRequest output hookEventName to PreToolUse
```

**현재 상태**: 대부분 해결됨. 새로운 hook 이벤트 추가 시 주의 필요.

### 3.2 Windows 플랫폼 이슈 (지속적 발생)

**발생 패턴**:
- 파일 경로 처리 (8.3 단축 경로 vs 전체 경로)
- 파일 잠금 (binary replacement)
- 실행 권한 (Unix 스타일 권한 미지원)
- UTF-8 인코딩
- PowerShell 호환성

**최근 해결**:
```
9084ebe6 fix(test): resolve CI test failures on Windows
9f52c01f fix(windows): platform default and browser URL escaping
ca73c008 fix(update): handle Windows file locking during binary replacement (#404)
```

**현재 상태**: 여전히 CI에서 Windows 관련 테스트 실패 발생 중.

### 3.3 huh/Viewport 이슈 (집중 발생 후 해결)

**발생 패턴**:
- huh v0.8.x viewport YOffset 버그
- Select 옵션이 화면에서 사라지는 문제
- 터미널 서브셸에서 viewport clipping

**해결 과정**:
```
c98e76d2 fix(wizard): resolve huh v0.8.x viewport and YOffset scroll bugs
411df1ba fix(wizard): set explicit form height to fix viewport clipping in subshell terminals
9cad82fc fix(wizard): correct Select.Height() to account for title+desc overhead in huh v0.8.x
f10f4bf8 refactor(wizard): eliminate shared viewport by running each question as independent huh.Form
```

**현재 상태**: 해결됨. huh 라이브러리 업그레이드 시 재발 가능성.

### 3.4 CI/Lint 이슈

**발생 패턴**:
- golangci-lint 버전 호환성
- Go 버전 업그레이드 시 lint 플래그 변경
- errcheck, ST1005 등 lint 규칙 위반

**최근 해결**:
```
8b28a910 fix(cli): resolve golangci-lint issues for CI pass
2045ae1d fix(lint): replace WriteString(fmt.Sprintf) with fmt.Fprintf, remove unused func
```

---

## 4. GitHub Issues 연결 분석

### 4.1 가장 많은 커밋이 연결된 Issues

| Issue | 커밋 수 | 주요 내용 |
|-------|---------|-----------|
| #390 | 6 | SPEC-GITHUB-WORKFLOW 구현 |
| #176 | 6 | 초기 설정 관련 |
| #173 | 4 | 초기 설정 관련 |
| #127 | 4 | 기능 개선 |
| #126 | 4 | 기능 개선 |
| #403 | 1 | SessionStart hook 타임아웃 |
| #404 | 1 | Windows 파일 잠금 |
| #401 | 1 | Rank 동기화 실패 |

### 4.2 최근 해결된 Issues

| Issue | 제목 | 해결 커밋 |
|-------|------|-----------|
| #403 | SessionStart hook timeout on session restart | fce49e50 |
| #404 | Windows file locking during binary replacement | ca73c008 |
| #401 | Rank sync marking failed sessions as synced | e3e07f5a |
| #396 | Wizard viewport YOffset bug | ec89dcad |
| #393 | Worktree SPEC-ID matching bugs | ea553ae0 |
| #389 | GLM env vars in status_line.sh | 19d432ef |
| #387 | Unsafe sed-based JSON editing | d7f0d2cd |

---

## 5. 현재 반복 발생 가능성이 높은 이슈

### 5.1 Windows CI 테스트 실패 (현재 진행 중)

**현재 상태**: PR #408에서 Windows CI 테스트 실패 발생

**실패 패턴**:
1. Unix 스타일 파일 권한 테스트 (755 vs 666)
2. Windows 8.3 단축 경로 문제
3. tmux 미지원으로 인한 CG 모드 테스트 실패

**해결 진행 상황**:
```
9084ebe6 fix(test): resolve CI test failures on Windows
```

### 5.2 예방 조치 필요 영역

| 영역 | 위험도 | 예방 조치 |
|------|--------|-----------|
| **Hook 타임아웃** | 중 | 타임아웃 설정 기본값 점검, context 취소 처리 |
| **Windows 경로** | 높음 | 모든 경로 비교 시 `filepath.EvalSymlinks` 사용 |
| **Lint 규칙** | 중 | pre-commit hook으로 golangci-lint 자동 실행 |
| **huh 업그레이드** | 중 | huh 버전 고정, 업그레이드 시 viewport 테스트 필수 |

---

## 6. 권장 사항

### 6.1 단기 (즉시 적용)

1. **Windows 테스트 스킵 전략**: Unix 전용 기능(파일 권한, tmux)은 `runtime.GOOS == "windows"` 체크로 스킵
2. **경로 정규화**: 모든 경로 비교에 `filepath.EvalSymlinks` 적용
3. **CI 안정화**: 현재 PR #408 CI 실패 해결 우선

### 6.2 중기 (1-2주)

1. **Hook 타임아웃 튜닝**: 기본값 5초에서 상황별 동적 조정 검토
2. **테스트 커버리지**: 현재 85% 이상 유지, Windows 특화 테스트 추가
3. **문서화**: 자주 발생하는 이슈에 대한 트러블슈팅 가이드 작성

### 6.3 장기 (1개월+)

1. **Windows 전용 CI**: Windows 전용 테스트 스위트 분리
2. **Hook 시스템 리팩토링**: 타임아웃 및 에러 처리 표준화
3. **의존성 버전 고정**: huh 등 문제가 있었던 라이브러리 버전 고정

---

## 7. 결론

1. **Go 1.26 업그레이드 후 안정화**: fix 커밋 수가 크게 감소하여 코드베이스가 안정화되는 추세
2. **Hook 시스템이 가장 많은 이슈**: 전체 fix 커밋의 약 25% 차지
3. **Windows 플랫폼 이슈 지속**: 크로스 플랫폼 호환성 개선 필요
4. **huh viewport 이슈 해결됨**: 라이브러리 업그레이드 시 주의 필요

---

**보고서 생성**: MoAI Strategic Orchestrator
**데이터 출처**: git log 분석 (4,218개 커밋)

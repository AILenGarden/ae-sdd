# Clean Code Skills for Claude Code

A collection of 20 skills that teach Claude Code to apply TDD, Refactoring, SOLID, OOP, Functional Programming, Hexagonal Architecture, and Clean Code principles during code reviews, generation, and pair programming sessions.

## Skills

### TDD
| Skill | Description |
|-------|-------------|
| `tdd-red-green-refactor` | Guide the red-green-refactor cycle for test-driven development |
| `tdd-test-first` | Enforce writing tests before production code |
| `tdd-arrange-act-assert` | Structure tests using the Arrange-Act-Assert pattern |

### Refactoring
| Skill | Description |
|-------|-------------|
| `refactor-extract-method` | Identify and perform extract method refactorings |
| `refactor-rename` | Apply meaningful naming to variables, methods, and classes |
| `refactor-replace-conditional-with-polymorphism` | Replace complex conditionals with polymorphic dispatch |

### SOLID
| Skill | Description |
|-------|-------------|
| `solid-single-responsibility` | Ensure each class has one reason to change |
| `solid-open-closed` | Design modules open for extension, closed for modification |
| `solid-liskov-substitution` | Verify subtypes are substitutable for their base types |
| `solid-interface-segregation` | Keep interfaces small and client-specific |
| `solid-dependency-inversion` | Depend on abstractions, not concretions |

### OOP
| Skill | Description |
|-------|-------------|
| `oop-encapsulation` | Protect internal state and expose minimal public APIs |
| `oop-composition-over-inheritance` | Favor object composition over class inheritance |

### Functional Programming
| Skill | Description |
|-------|-------------|
| `fp-pure-functions` | Write pure functions with no side effects |
| `fp-higher-order-functions` | Use higher-order functions to reduce duplication |
| `fp-error-handling` | Handle errors functionally with Result/Either/Option types |

### Hexagonal Architecture
| Skill | Description |
|-------|-------------|
| `hexagonal-ports-adapters` | Structure code using ports and adapters |
| `hexagonal-domain-isolation` | Isolate domain logic from infrastructure concerns |

### Clean Code
| Skill | Description |
|-------|-------------|
| `clean-code-boy-scout-rule` | Leave code cleaner than you found it |
| `clean-code-detect-smells` | Detect and categorize code smells |

## Installation

### Global (all projects)

```bash
./install.sh --global
```

### Per-project

```bash
./install.sh --project /path/to/your/project
```

### Single skill

```bash
./install.sh --skill tdd-red-green-refactor --global
./install.sh --skill solid-single-responsibility --project /path/to/project
```

### List available skills

```bash
./install.sh --list
```

### Uninstall

```bash
./install.sh --uninstall --global
./install.sh --uninstall --project /path/to/your/project
```

## Slash Command

After installation, use `/clean-code-audit` in Claude Code to run a comprehensive code quality audit against all 20 skills.

## License

MIT

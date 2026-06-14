---
name: requirements-tracker
description: Track feature requirements and ensure new changes don't break existing functionality. Use when the user describes new features, makes functional requests, or asks to implement changes to the coding-plan-widget project.
---

# Requirements Tracker

This skill maintains a living requirements document for the coding-plan-widget project. Its purpose is to ensure that when new features are implemented, existing functionality is not broken.

## When to Use This Skill

Activate this skill whenever:
- The user describes a new feature or functional requirement
- The user asks to implement a change or new functionality
- The user reports a bug or requests a behavior change
- You are about to make code changes that affect behavior

## Instructions

### 1. Read the Requirements Document First

At the start of every conversation where this skill is activated, read the requirements document:

```
.agents/skills/requirements-tracker/requirements.md
```

This gives you a complete picture of all previously recorded requirements and ensures you don't accidentally break anything.

### 2. Record New Requirements

When the user describes a specific feature, behavior, or functional requirement, append it to `requirements.md` using the following format:

```markdown
## [Feature Name]

**Date**: YYYY-MM-DD
**Status**: ✅ Implemented / 🔧 In Progress / 📋 Planned

### Description
[Clear description of what the feature does]

### Acceptance Criteria
- [ ] [Specific, testable criterion]
- [ ] [Specific, testable criterion]

### Implementation Notes
- [Relevant file paths, key functions, design decisions]
```

### 3. Check Before Implementing

Before making any code changes:
1. Read the full `requirements.md`
2. Identify which existing requirements could be affected by your changes
3. Explicitly state in your response: "I've reviewed the requirements and these existing features could be affected: [list]"
4. After implementing, verify that existing features still work as described

### 4. Update Status

When a feature is fully implemented, update its status to `✅ Implemented`. When you start working on it, update to `🔧 In Progress`.

### 5. Keep It Concise

- Don't duplicate information — each requirement should be recorded once
- Focus on behavior and user-facing functionality, not implementation details
- Keep acceptance criteria testable and specific

# Sensei Agent Workflows

This document defines the automated agent workflows for maintaining code quality.

## PR Review Agent

**Trigger:** New PR opened or updated

**Workflow:**
1. Wait for CI checks to complete
2. Review code changes for:
   - Adherence to Rust idioms
   - Error handling correctness
   - Test coverage for new code
   - Documentation completeness
   - Performance considerations
3. Provide inline comments for improvements
4. Approve or request changes

**Commands:**
- Review: `@claude review this PR`
- Simplify: `@claude simplify this code`

## DeepSource Remediation Agent

**Trigger:** DeepSource reports issues on PR

**Workflow:**
1. Wait 60 seconds after PR CI begins (allow DeepSource scan)
2. Fetch DeepSource issues via MCP
3. For each issue:
   - Understand the issue context
   - Apply fix
   - Verify fix doesn't break tests
4. Commit fixes with message: `fix: resolve DeepSource issue [ISSUE_ID]`
5. Push updated branch
6. If new issues appear, repeat

**Exit Condition:** DeepSource shows 0 issues on PR

## Code Simplification Agent

**Trigger:** Manual invocation or large PR

**Workflow:**
1. Identify complex functions (cognitive complexity > 10)
2. Propose refactoring:
   - Extract helper functions
   - Reduce nesting
   - Simplify conditionals
3. Ensure tests still pass
4. Commit simplifications separately from feature code

## Pre-Merge Checklist Agent

**Trigger:** PR approved, ready to merge

**Verification:**
- [ ] All CI checks pass
- [ ] Coverage >= 95%
- [ ] DeepSource shows 0 issues
- [ ] No unresolved review comments
- [ ] CHANGELOG.md updated (if user-facing change)
- [ ] Version bumped (if releasing)

**Action:** Merge via squash commit with conventional commit message

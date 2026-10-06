# Claude Design System Installed

## Installation Status

| Component | Status |
|-----------|--------|
| Design Document (DESIGN.md) | Installed |
| Skill File | Installed |
| Permissions Config | Updated |

## File Locations

```
.claude/
├── settings.local.json          (permissions updated)
└── skills/
    ├── frontend-design.md       (main skill file)
    └── frontend-design.yaml     (auxiliary config)
```

## Design System Summary

### Style Transformation
| Dimension | Original Cinematic | New Literary Salon |
|-----------|-------------------|-------------------|
| Background | #0a0a0f deep black | #f5f4ed parchment |
| Accent | #d4af37 gold | #c96442 terracotta |
| Feel | Hollywood/cinema | Intellectual/literary |
| Font | Cinzel decorative | Georgia classic |

### Core Colors
- Primary: #141413 (anthropic-black), #c96442 (terracotta)
- Background: #f5f4ed (parchment), #faf9f5 (ivory)
- Text: #5e5d59 (olive-gray), #87867f (stone-gray)

### Typography
- Headlines: Georgia, serif, weight 500 (no bold!)
- Body: system-ui, sans-serif
- Line-height: 1.60 (generous reading)

### Component Patterns
```tsx
// Primary Button
<button className="bg-[#c96442] text-[#faf9f5] px-4 py-2 rounded-lg shadow-[0px_0px_0px_1px_#c96442]">

// Card
<div className="bg-[#faf9f5] border border-[#f0eee6] rounded-2xl p-6">

// Input
<input className="bg-white border border-[#e8e6dc] rounded-xl focus:border-[#3898ec]" />
```

## How to Use

When you ask Claude to design frontend interfaces, it will automatically use this design system:

Examples:
- "Create a story list page"
- "Design a settings panel component"
- "Generate chapter editor UI"

Claude will generate code following these specifications:
- Parchment background (#f5f4ed)
- Terracotta primary buttons (#c96442)
- Georgia serif headlines
- Generous line-height (1.60)
- Rounded corners (6px+)
- Ring shadow effects

## Design Principles

1. Warm: All grays have yellow-brown undertones
2. Intellectual: Serif headlines + generous typography
3. Clean: No gradients, no heavy shadows
4. Organic: Hand-drawn style illustrations

## Next Steps

1. Ask Claude to refactor existing pages with new design
2. Update tailwind.config.js colors
3. Update global styles in index.css
4. Redesign component library

---

Status: Design system installed and ready
Location: .claude/skills/frontend-design.md
Version: v2.0 - Literary Salon Style

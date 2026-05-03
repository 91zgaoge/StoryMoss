pub const MARKDOWN_DEFAULT: &str = r##"# {{ story.title }}

{% if config.include_metadata %}
## 信息

{% if story.genre %}- **类型**: {{ story.genre }}{% endif %}
{% if story.tone %}- **基调**: {{ story.tone }}{% endif %}
{% if story.pacing %}- **节奏**: {{ story.pacing }}{% endif %}
- **章节数**: {{ chapters | length }}

{% if story.description %}
## 简介

{{ story.description }}

{% endif %}
{% endif %}
{% if config.include_metadata and characters %}
## 人物介绍

{% for character in characters %}
### {{ character.name }}

{% if character.background %}{{ character.background }}
{% endif %}
{% if character.personality %}**性格**: {{ character.personality }}
{% endif %}
{% if character.goals %}**目标**: {{ character.goals }}
{% endif %}

{% endfor %}
{% endif %}
---

# 正文

{% for chapter in chapters %}
## {{ chapter.title | default(value="未命名章节") }}

{% if config.include_outline and chapter.outline %}**大纲**: {{ chapter.outline }}

{% endif %}
{{ chapter.content | default(value="") }}

---

{% endfor %}
"##;

pub const HTML_ELEGANT: &str = r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>{{ story.title }}</title>
<style>
body { font-family: "Noto Serif SC", Georgia, serif; line-height: 1.8; max-width: 800px; margin: 0 auto; padding: 2em; background: #fafafa; color: #333; }
h1 { text-align: center; font-size: 2.5em; margin-bottom: 0.5em; color: #222; }
h2 { font-size: 1.8em; margin-top: 2em; color: #444; border-bottom: 1px solid #ddd; padding-bottom: 0.3em; }
h3 { font-size: 1.3em; color: #555; }
p { text-indent: 2em; margin: 1em 0; }
.metadata { background: #f0f0f0; padding: 1em; border-radius: 8px; margin: 1em 0; }
.outline { font-style: italic; color: #666; background: #f9f9f9; padding: 1em; border-left: 3px solid #999; }
.character { margin: 1em 0; padding: 1em; background: #fff; border: 1px solid #e0e0e0; border-radius: 8px; }
hr { border: none; border-top: 1px solid #ddd; margin: 2em 0; }
.toc { background: #f5f5f5; padding: 1em; border-radius: 8px; margin-bottom: 2em; }
.toc a { color: #333; text-decoration: none; }
.toc a:hover { text-decoration: underline; }
</style>
</head>
<body>
<h1>{{ story.title | escape }}</h1>

{% if config.include_metadata %}
<div class="metadata">
{% if story.genre %}<p><strong>类型</strong>: {{ story.genre | escape }}</p>{% endif %}
{% if story.tone %}<p><strong>基调</strong>: {{ story.tone | escape }}</p>{% endif %}
<p><strong>章节数</strong>: {{ chapters | length }}</p>
</div>

{% if story.description %}<p>{{ story.description | escape }}</p>{% endif %}

{% if characters %}
<h2>人物介绍</h2>
{% for character in characters %}
<div class="character">
<h3>{{ character.name | escape }}</h3>
{% if character.background %}<p>{{ character.background | escape }}</p>{% endif %}
</div>
{% endfor %}
{% endif %}
{% endif %}

<div class="toc">
<h2>目录</h2>
<ol>
{% for chapter in chapters %}
<li><a href="#chapter-{{ loop.index }}">{{ chapter.title | default(value="未命名章节") | escape }}</a></li>
{% endfor %}
</ol>
</div>

<hr>
<h2>正文</h2>

{% for chapter in chapters %}
<h3 id="chapter-{{ loop.index }}">{{ chapter.title | default(value="未命名章节") | escape }}</h3>
{% if config.include_outline and chapter.outline %}
<div class="outline">大纲: {{ chapter.outline | escape }}</div>
{% endif %}
{% if chapter.content %}
{% for para in chapter.content | split(pat="\n\n") %}
{% if para | trim %}
<p>{{ para | trim | escape }}</p>
{% endif %}
{% endfor %}
{% endif %}
<hr>
{% endfor %}

</body>
</html>
"##;

pub const TXT_PLAIN: &str = r##"{{ story.title }}
{{ "=" | repeat(n=story.title | length) }}

{% if config.include_metadata %}
{% if story.genre %}类型: {{ story.genre }}{% endif %}
{% if story.tone %}基调: {{ story.tone }}{% endif %}
章节数: {{ chapters | length }}

{% if story.description %}
简介
{{ "-" | repeat(n=20) }}
{{ story.description }}

{% endif %}
{% endif %}
{% if config.include_metadata and characters %}
人物介绍
{{ "-" | repeat(n=20) }}
{% for character in characters %}

{{ character.name }}
{% if character.background %}{{ character.background }}{% endif %}
{% endfor %}

{% endif %}
正文
{{ "=" | repeat(n=40) }}

{% for chapter in chapters %}
{{ chapter.title | default(value="未命名章节") }}
{{ "-" | repeat(n=chapter.title | default(value="未命名章节") | length) }}

{% if config.include_outline and chapter.outline %}[大纲]: {{ chapter.outline }}

{% endif %}
{{ chapter.content | default(value="") }}

{% endfor %}
"##;

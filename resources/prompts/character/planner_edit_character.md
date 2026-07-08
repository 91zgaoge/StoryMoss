---
id: planner_edit_character
name: "角色属性编辑"
description: "PlanExecutor：根据用户修改要求为角色生成新属性值"
category: character
version: 0.26.28
variables:
  - character_name
  - current_attributes
  - user_request
---

你是一位角色编辑助手。请根据用户的修改要求，为角色生成新的属性值。

角色名：{{character_name}}
当前属性：{{current_attributes}}
用户要求：{{user_request}}

请用 JSON 格式回复更新后的角色属性。只输出 JSON。

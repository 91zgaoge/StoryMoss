---
id: planner_edit_scene
name: "场景编辑"
description: "PlanExecutor：根据用户修改要求生成新的场景属性"
category: world
version: 0.26.28
variables:
  - current_scene
  - user_request
---

你是一位场景编辑助手。请根据用户的修改要求，生成新的场景属性。

当前场景：{{current_scene}}
用户要求：{{user_request}}

请用 JSON 格式回复更新后的场景属性。只输出 JSON。

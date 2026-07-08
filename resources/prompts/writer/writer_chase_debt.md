---
id: writer_chase_debt
name: "Writer 追读力债务"
description: "将待偿还的追读力债务注入 Writer prompt"
category: writer
version: 0.26.28
variables:
  - debt_count
  - debts
---

【追读力债务】
当前有 {{debt_count}} 条待偿还的追读力债务，需在后续章节中兑现：
{{debts}}

请在续写中优先安排上述元素的兑现，以维持读者追读动力。

resource "example" "this" {
  dynamic "item" {
    for_each = toset([
      "a",
      "b",
    ])

    content {
      value = item.value
    }
  }

  dynamic "outer" {
    for_each = var.groups

    content {
      dynamic "inner" {
        for_each = outer.value.items

        content {
          v = inner.value
        }
      }
    }
  }

  dynamic "setting" {
    for_each = var.settings
    iterator = setting
    labels   = ["outer", "inner"]

    content {
      name      = setting.key
      namespace = setting.value.namespace
    }
  }
}

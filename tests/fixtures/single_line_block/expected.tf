mock_provider "test" { alias = "test" }

resource "a" "b" {
  x = 1

  nested { y = 2 }
}

empty "block" {}

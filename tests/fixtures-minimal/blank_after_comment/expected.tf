# lead


resource "a" "b" {
  o = {
    a = 1

    # c

    b = 2
  }
  l = [
    1,

    # c

    2,
  ]
  x = 1

  # header

  y = 2
  # hug
  z = 3

  nested {
    a = 1
  }

  # nested header

  nested {
    a = 2
  }
}

# top header

resource "c" "d" {
}

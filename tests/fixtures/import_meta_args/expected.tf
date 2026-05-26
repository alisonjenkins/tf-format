import {
  for_each = toset(["one", "two"])
  provider = aws.east

  id = "i-abcd1234"
  to = aws_instance.example
}

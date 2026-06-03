data "aws_ami" "a" {
  id = 3
}

data "aws_ami" "z" {
  id = 2
}

data "aws_subnet" "b" {
  id = 1
}

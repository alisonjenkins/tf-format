resource "aws_instance" "web" {
  ami = "a"
}

resource "aws_instance" "api" {
  ami = "b"
}

resource "aws_instance" "db" {
  ami = "c"
}

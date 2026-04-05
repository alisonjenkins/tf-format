resource "aws_instance" "example" {
  depends_on = [
    aws_security_group.example,
    aws_vpc.main,
  ]

  ami           = "ami-12345678"
  instance_type = "t2.micro"
}

resource "aws_security_group" "example" {
  name = "example"

  ingress {
    from_port = 443
    protocol  = "tcp"
    to_port   = 443

    cidr_blocks = [
      "10.0.0.0/8",
      "172.16.0.0/12",
    ]
  }
}

resource "aws_instance" "x" {
  count      = 2
  provider   = aws.east
  depends_on = [aws_vpc.main]

  ami = "ami-1"
}

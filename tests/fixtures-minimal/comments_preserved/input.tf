resource "aws_instance" "x" {
  ami = "ami-123"
  # break alignment here
  instance_type = "t3.micro"
  key_name = "deployer"
}

output "instance_ip" {
  description = "Public IP of the instance"
  value       = aws_instance.example.public_ip
  depends_on  = [aws_instance.example]
  sensitive   = false
}

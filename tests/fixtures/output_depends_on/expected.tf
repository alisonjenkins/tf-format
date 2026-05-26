output "instance_ip" {
  depends_on = [aws_instance.example]

  description = "Public IP of the instance"
  sensitive   = false
  value       = aws_instance.example.public_ip
}

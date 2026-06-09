#!/usr/bin/env python3
"""
PyTorch Reference: MNIST Training with Precision/Recall/F1 Metrics

Trains the same 784 → 128 → 10 architecture on MNIST and computes
comprehensive metrics for comparison with Salt training.
"""

import time
import numpy as np
import torch
import torch.nn as nn
import torch.optim as optim

from pathlib import Path

# Hyperparameters (matching C baseline)
MAX_EPOCHS = 5
LEARNING_RATE = 0.001  # Unified across all implementations
HIDDEN_SIZE = 128

class SimpleNet(nn.Module):
    """Same architecture as keuos_train.salt"""
    def __init__(self):
        super().__init__()
        self.fc1 = nn.Linear(784, HIDDEN_SIZE, bias=True)
        self.fc2 = nn.Linear(HIDDEN_SIZE, 10, bias=True)
        
        # Xavier initialization (matching Salt)
        nn.init.xavier_uniform_(self.fc1.weight)
        nn.init.xavier_uniform_(self.fc2.weight)
        nn.init.zeros_(self.fc1.bias)
        nn.init.zeros_(self.fc2.bias)
    
    def forward(self, x):
        x = torch.relu(self.fc1(x))
        x = self.fc2(x)
        return x

def load_mnist_binary():
    """Load MNIST from binary files (same format as Salt)"""
    data_dir = Path(__file__).parent
    
    # Load training data (f32 format)
    train_images = np.fromfile(data_dir / "mnist_train_images.bin", dtype=np.float32)
    train_images = train_images.reshape(60000, 784)
    train_labels = np.fromfile(data_dir / "mnist_train_labels.bin", dtype=np.uint8)
    
    # Load test data
    test_images = np.fromfile(data_dir / "mnist_test_images.bin", dtype=np.float32)
    test_images = test_images.reshape(10000, 784)
    test_labels = np.fromfile(data_dir / "mnist_test_labels.bin", dtype=np.uint8)
    
    return (
        torch.from_numpy(train_images),
        torch.from_numpy(train_labels.astype(np.int64)),
        torch.from_numpy(test_images),
        torch.from_numpy(test_labels.astype(np.int64))
    )

def train_epoch(model, images, labels, optimizer, criterion):
    """Train one epoch with online (batch_size=1) learning"""
    model.train()
    correct = 0
    
    for i in range(len(images)):
        x = images[i:i+1]
        y = labels[i:i+1]
        
        optimizer.zero_grad()
        output = model(x)
        loss = criterion(output, y)
        loss.backward()
        optimizer.step()
        
        pred = output.argmax(dim=1)
        correct += (pred == y).sum().item()
    
    return correct / len(images)

def evaluate(model, images, labels):
    """Evaluate and compute precision/recall/F1"""
    model.eval()
    all_preds = []
    all_labels = []
    
    with torch.no_grad():
        for i in range(len(images)):
            x = images[i:i+1]
            output = model(x)
            pred = output.argmax(dim=1).item()
            all_preds.append(pred)
            all_labels.append(labels[i].item())
    
    # Compute metrics
    precision = np.zeros(10)
    recall = np.zeros(10)
    f1 = np.zeros(10)
    
    for c in range(10):
        tp = sum(1 for p, l in zip(all_preds, all_labels) if p == c and l == c)
        fp = sum(1 for p, l in zip(all_preds, all_labels) if p == c and l != c)
        fn = sum(1 for p, l in zip(all_preds, all_labels) if p != c and l == c)
        
        precision[c] = tp / (tp + fp) if (tp + fp) > 0 else 0.0
        recall[c] = tp / (tp + fn) if (tp + fn) > 0 else 0.0
        f1[c] = 2 * (precision[c] * recall[c]) / (precision[c] + recall[c]) if (precision[c] + recall[c]) > 0 else 0.0
    
    # Macro averages
    macro_precision = np.mean(precision)
    macro_recall = np.mean(recall)
    macro_f1 = np.mean(f1)
    
    accuracy = sum(1 for p, l in zip(all_preds, all_labels) if p == l) / len(all_labels)
    
    return {
        'accuracy': accuracy,
        'precision_per_class': precision,
        'recall_per_class': recall,
        'f1_per_class': f1,
        'macro_precision': macro_precision,
        'macro_recall': macro_recall,
        'macro_f1': macro_f1,
        'predictions': all_preds,
        'labels': all_labels
    }

def main():
    print("=" * 60)
    print("PyTorch MNIST Benchmark (Same Architecture as Salt)")
    print("=" * 60)
    
    # Load data
    print("\nLoading MNIST from binary files...")
    train_images, train_labels, test_images, test_labels = load_mnist_binary()
    print(f"  Train: {len(train_images)} samples")
    print(f"  Test:  {len(test_images)} samples")
    
    # Initialize model
    model = SimpleNet()
    criterion = nn.CrossEntropyLoss()
    optimizer = optim.SGD(model.parameters(), lr=LEARNING_RATE)
    
    print(f"\nModel: 784 → {HIDDEN_SIZE} (ReLU) → 10")
    print(f"Params: {sum(p.numel() for p in model.parameters()):,}")
    print(f"Learning Rate: {LEARNING_RATE:.2e}")
    
    # Training
    print("\n" + "-" * 40)
    print("Training (Online SGD, batch_size=1)")
    print("-" * 40)
    
    total_start = time.time()
    
    # Convergence tracking
    prev_accuracy = 0
    plateau_count = 0
    epoch = 0
    
    while epoch < MAX_EPOCHS and plateau_count < 5:
        epoch_start = time.time()
        accuracy = train_epoch(model, train_images, train_labels, optimizer, criterion)
        epoch_time = time.time() - epoch_start
        accuracy_pct = int(accuracy * 100)
        print(f"Epoch {epoch+1}: {accuracy_pct}% accuracy ({epoch_time*1000:.0f} ms)")
        
        # Convergence check: stop if <1% improvement for 5 epochs
        if accuracy_pct - prev_accuracy < 1:
            plateau_count += 1
        else:
            plateau_count = 0
        prev_accuracy = accuracy_pct
        epoch += 1
    
    if plateau_count >= 5:
        print("Converged (plateau detected)")
    
    total_time = time.time() - total_start
    print(f"\nTotal training time: {total_time*1000:.0f} ms")
    
    # Evaluation
    print("\n" + "-" * 40)
    print("Evaluation on Test Set")
    print("-" * 40)
    
    metrics = evaluate(model, test_images, test_labels)
    
    print(f"\nOverall Accuracy: {metrics['accuracy']*100:.2f}%")
    print(f"\nPer-Class Metrics:")
    print(f"{'Class':<8} {'Precision':<12} {'Recall':<12} {'F1':<12}")
    print("-" * 44)
    
    for i in range(10):
        print(f"{i:<8} {metrics['precision_per_class'][i]:.4f}       "
              f"{metrics['recall_per_class'][i]:.4f}       "
              f"{metrics['f1_per_class'][i]:.4f}")
    
    print("-" * 44)
    print(f"{'Macro':<8} {metrics['macro_precision']:.4f}       "
          f"{metrics['macro_recall']:.4f}       "
          f"{metrics['macro_f1']:.4f}")
    
    print("\n" + "=" * 60)
    print("Summary for Comparison with Salt")
    print("=" * 60)
    print(f"Training Time:    {total_time*1000:.0f} ms")
    print(f"Test Accuracy:    {metrics['accuracy']*100:.2f}%")
    print(f"Macro F1 Score:   {metrics['macro_f1']:.4f}")
    print("=" * 60)

if __name__ == "__main__":
    main()

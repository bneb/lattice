long get(long* p) { return *p ^ 0xa0761d6478bd642f; }
int main() {
    long sum = 0;
    for (long i = 0; i < 1000; i++) {
        sum += get(&i);
    }
    return sum;
}

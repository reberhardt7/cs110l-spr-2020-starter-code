#include <stdio.h>

int global = 5;

void func3(int a) {
    printf("Hello from func3! %d\n", a);
}

void func2(int a, int b) {
    printf("func2(%d, %d) was called\n", a, b);
    int sum = a + b;
    printf("sum = %d\n", sum);
    func3(100);
}

void func1(int a) {
    printf("func1(%d) was called\n", a);
    func2(a, global);
    func3(100);
    printf("end of func1\n");
}

int main() {
    func1(42);
}

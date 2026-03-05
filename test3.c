int g_counter;

void inc() {
    g_counter = g_counter + 1;
}

int sum_array(int n) {
    int arr[8];
    int i;
    int total;
    i = 0;
    total = 0;
    while (i < n) {
        arr[i] = i * 2;
        total = total + arr[i];
        i = i + 1;
    }
    return total;
}

int main() {
    int s;
    g_counter = 0;
    inc();
    inc();
    s = sum_array(4);
    return s + g_counter;
}

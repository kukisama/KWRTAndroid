# 保留默认行为 + 针对本项目的最小集
# OkHttp / Okio
-dontwarn okhttp3.**
-dontwarn okio.**
-dontwarn org.conscrypt.**
-dontwarn org.bouncycastle.**
-dontwarn org.openjsse.**

# Kotlin metadata 已由 AGP 默认规则处理；Compose 也是。
# 我们没有反射加载的类，不需要额外 -keep。

# Technology

Integrating existing technology might save us from reinventing the wheel too much. At the moment, using an existing solution to execute concrete chunks of build work looks like it will provide the biggest benefits.

- [Buildbot](https://buildbot.net/) ([docs](https://docs.buildbot.net/current/))
    - Configured in Python
    - Very flexible
    - Python updates can be problematic
- [Dagger](https://dagger.io/) 
    - Configured in Python, TypeScript or Go
    - Very flexible
    - Trying very hard to be a platform (See "daggerverse"). This could get in our way
    - no self-hosting docs?
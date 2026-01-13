# Timely Pass: Time-based Password manager

Timely Pass allows the user to set dynamic passwords with custom policies on their computer based on time constraitns.

## Hooks

1. onlyBeforeTime: Only accept this password before `onlyBeforeTime: TIME` time.
2. onlyAfterTime: Only accept this password after `onlyAfterTime: TIME` time.
3. onlyWithinTime: Only accept this password within `onlyWithinTime: TIME` period of time.
4. onlyForTime: Only accept this password for `onlyForTime: TIME` period of time.
5. onlyBeforeDate: Only accept this password before `onlyBeforeDate: DATE` day.
6. onlyAfterDate: Only accept this password after `onlyAfterDate: DATE` day.
7. onlyWithinDate: Only accept this password within `onlyWithinDate: DATE` period of time (DATE).
8. onlyForDate: Only accept this password for `onlyBeforeTime: DATE` period of time (DATE).

How it works: Time (at so so time/date, rotate acceptable password to only this password/mechanism).

# By [Balqaasem](https://balqaasem.xyz)
